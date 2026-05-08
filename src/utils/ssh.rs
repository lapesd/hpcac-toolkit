use anyhow::Result;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{Duration, Instant, sleep};

pub struct SshSession {
    pub ip: String,
    pub username: String,
    pub private_key_path: String,
}

impl SshSession {
    pub fn for_aws(ip: &str, private_key_path: &str) -> Self {
        Self {
            ip: ip.to_string(),
            username: "ec2-user".to_string(),
            private_key_path: private_key_path.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn for_vultr(ip: &str, private_key_path: &str) -> Self {
        Self {
            ip: ip.to_string(),
            username: "root".to_string(),
            private_key_path: private_key_path.to_string(),
        }
    }

    fn base_args(&self) -> Vec<String> {
        vec![
            "-i".to_string(),
            self.private_key_path.clone(),
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-o".to_string(),
            "ServerAliveInterval=30".to_string(),
            "-o".to_string(),
            "ServerAliveCountMax=3".to_string(),
            "-o".to_string(),
            "LogLevel=ERROR".to_string(),
            format!("{}@{}", self.username, self.ip),
        ]
    }

    /// Poll port 22 until the instance accepts TCP connections or the timeout is reached.
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let addr = format!("{}:22", self.ip);
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                anyhow::bail!(
                    "Timed out after {:?} waiting for SSH on '{}'",
                    timeout,
                    self.ip
                );
            }
            // Per-attempt timeout so dropped packets (SG not yet open, instance booting)
            // don't block on the kernel's multi-minute SYN retransmit timer.
            match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await {
                Ok(Ok(_)) => {
                    tracing::info!(
                        "SSH port open on '{}', waiting for sshd to initialize...",
                        self.ip
                    );
                    sleep(Duration::from_secs(5)).await;
                    return Ok(());
                }
                Ok(Err(_)) | Err(_) => {
                    tracing::info!("SSH not yet ready on '{}', retrying in 5s...", self.ip);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Run a shell script on the remote host, blocking until it completes.
    /// The script is piped to `bash` via stdin — no quoting issues regardless of content.
    pub async fn run_command(&self, script: &str) -> Result<String> {
        tracing::info!("Running SSH command on '{}'", self.ip);
        let mut args = self.base_args();
        args.push("bash -s".to_string());

        let mut child = Command::new("ssh")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(script.as_bytes()).await?;
        }

        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            if !stdout.trim().is_empty() {
                tracing::error!("SSH stdout: {}", stdout);
            }
            anyhow::bail!("SSH command failed on '{}': {}", self.ip, stderr);
        }

        Ok(stdout)
    }

    /// Upload raw bytes to a file on the remote host via stdin.
    /// The content is piped directly — no encoding or quoting needed.
    pub async fn upload_file(&self, remote_path: &str, content: &str) -> Result<()> {
        tracing::info!("Uploading file to '{}:{}'", self.ip, remote_path);
        let mut args = self.base_args();
        args.push(format!("cat > {}", remote_path));

        let mut child = Command::new("ssh")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(content.as_bytes()).await?;
        }

        let output = child.wait_with_output().await?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to upload file to '{}:{}': {}",
                self.ip,
                remote_path,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Start a script in a detached tmux session. Returns immediately.
    /// The script is uploaded as a temp file to avoid any quoting issues.
    #[allow(dead_code)]
    pub async fn run_in_tmux(&self, session_name: &str, script: &str) -> Result<()> {
        let script_path = format!("/tmp/hpcac_{}.sh", session_name);
        let log_path = format!("/tmp/hpcac_{}.log", session_name);

        self.upload_file(&script_path, script).await?;
        self.run_command(&format!(
            "chmod +x {script_path} && tmux new-session -d -s {session_name} \
            '{script_path} 2>&1 | tee {log_path}'"
        ))
        .await?;

        tracing::info!(
            "tmux session '{}' started on '{}'. Log: {}",
            session_name,
            self.ip,
            log_path
        );
        Ok(())
    }

    /// Check whether a tmux session with the given name is still running.
    #[allow(dead_code)]
    pub async fn tmux_session_exists(&self, session_name: &str) -> Result<bool> {
        let result = self
            .run_command(&format!(
                "tmux has-session -t {} 2>/dev/null && echo running || echo done",
                session_name
            ))
            .await?;
        Ok(result.trim() == "running")
    }

    /// Block until the tmux session exits, polling at the given interval.
    #[allow(dead_code)]
    pub async fn wait_for_tmux(&self, session_name: &str, poll_interval: Duration) -> Result<()> {
        loop {
            if !self.tmux_session_exists(session_name).await? {
                tracing::info!(
                    "tmux session '{}' on '{}' has finished",
                    session_name,
                    self.ip
                );
                return Ok(());
            }
            tracing::info!(
                "tmux session '{}' on '{}' still running...",
                session_name,
                self.ip
            );
            sleep(poll_interval).await;
        }
    }
}
