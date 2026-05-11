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
    pub jump_host: Option<String>,
}

impl SshSession {
    fn display(&self) -> String {
        match &self.jump_host {
            Some(jump) => format!("{} (via {})", self.ip, jump),
            None => self.ip.clone(),
        }
    }
}

impl SshSession {
    pub fn for_aws(ip: &str, private_key_path: &str) -> Self {
        Self {
            ip: ip.to_string(),
            username: "ec2-user".to_string(),
            private_key_path: private_key_path.to_string(),
            jump_host: None,
        }
    }

    pub fn for_aws_worker(private_ip: &str, head_public_ip: &str, private_key_path: &str) -> Self {
        Self {
            ip: private_ip.to_string(),
            username: "ec2-user".to_string(),
            private_key_path: private_key_path.to_string(),
            jump_host: Some(format!("ec2-user@{}", head_public_ip)),
        }
    }

    #[allow(dead_code)]
    pub fn for_vultr(ip: &str, private_key_path: &str) -> Self {
        Self {
            ip: ip.to_string(),
            username: "root".to_string(),
            private_key_path: private_key_path.to_string(),
            jump_host: None,
        }
    }

    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
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
        ];
        if let Some(ref jump) = self.jump_host {
            // Use ProxyCommand instead of ProxyJump so we can pass StrictHostKeyChecking=no
            // and the identity file to the jump hop as well.
            args.push("-o".to_string());
            args.push(format!(
                "ProxyCommand=ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -W %h:%p {}",
                self.private_key_path, jump
            ));
        }
        args.push(format!("{}@{}", self.username, self.ip));
        args
    }

    /// Wait until the instance is reachable via SSH.
    /// For sessions with a jump host, uses SSH command probing (can't TCP-poll private IPs).
    /// For direct sessions, polls TCP port 22.
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        if self.jump_host.is_some() {
            return self.wait_until_ready_via_ssh(timeout).await;
        }
        let addr = format!("{}:22", self.ip);
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                anyhow::bail!(
                    "Timed out after {:?} waiting for SSH on '{}'",
                    timeout,
                    self.display()
                );
            }
            // Per-attempt timeout so dropped packets (SG not yet open, instance booting)
            // don't block on the kernel's multi-minute SYN retransmit timer.
            match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await {
                Ok(Ok(_)) => {
                    tracing::info!(
                        "SSH port open on '{}', waiting for sshd to initialize...",
                        self.display()
                    );
                    sleep(Duration::from_secs(5)).await;
                    return Ok(());
                }
                Ok(Err(_)) | Err(_) => {
                    tracing::info!("SSH not yet ready on '{}', retrying in 5s...", self.display());
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Poll via SSH command for sessions that go through a jump host (private IPs).
    async fn wait_until_ready_via_ssh(&self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                anyhow::bail!(
                    "Timed out after {:?} waiting for SSH on '{}'",
                    timeout,
                    self.display()
                );
            }
            match self.run_command("echo ok").await {
                Ok(_) => {
                    tracing::info!("SSH ready on '{}'", self.display());
                    return Ok(());
                }
                Err(_) => {
                    tracing::info!("SSH not yet ready on '{}', retrying in 5s...", self.display());
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Run a shell script on the remote host, blocking until it completes.
    /// The script is piped to `bash` via stdin — no quoting issues regardless of content.
    pub async fn run_command(&self, script: &str) -> Result<String> {
        tracing::info!("Running SSH command on '{}'", self.display());
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
            anyhow::bail!("SSH command failed on '{}': {}", self.display(), stderr);
        }

        Ok(stdout)
    }

    /// Run a shell script on the remote host, streaming each output line to the
    /// terminal in real time. Fails immediately if the remote script exits non-zero.
    pub async fn run_command_streaming(&self, script: &str) -> Result<()> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        tracing::info!("Running SSH command on '{}'", self.display());
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

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let node = self.display();
        let node2 = node.clone();

        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[{}] {}", node, line);
            }
        });
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[{}] {}", node2, line);
            }
        });

        let status = child.wait().await?;
        let _ = tokio::join!(stdout_task, stderr_task);

        if !status.success() {
            anyhow::bail!(
                "Init command failed on '{}' with exit code {}",
                self.display(),
                status.code().unwrap_or(-1)
            );
        }

        Ok(())
    }

    /// Upload raw bytes to a file on the remote host via stdin.
    /// The content is piped directly — no encoding or quoting needed.
    pub async fn upload_file(&self, remote_path: &str, content: &str) -> Result<()> {
        tracing::info!("Uploading file to '{}:{}'", self.display(), remote_path);
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
    /// A wrapper script captures the exit code to `{script_path}.exit` so
    /// callers can detect failure after the session ends.
    #[allow(dead_code)]
    pub async fn run_in_tmux(&self, session_name: &str, script: &str) -> Result<()> {
        let script_path = format!("/tmp/hpcac_{}.sh", session_name);
        let log_path = format!("/tmp/hpcac_{}.log", session_name);
        let exit_path = format!("/tmp/hpcac_{}.exit", session_name);
        let wrapper_path = format!("/tmp/hpcac_{}_wrapper.sh", session_name);

        // Wrapper runs the script, tees output to the log, and saves the exit code.
        // PIPESTATUS[0] captures the script's exit code despite the tee pipeline.
        let wrapper = format!(
            "#!/bin/bash\n{script_path} 2>&1 | tee {log_path}\necho ${{PIPESTATUS[0]}} > {exit_path}\n"
        );

        self.upload_file(&script_path, script).await?;
        self.upload_file(&wrapper_path, &wrapper).await?;
        self.run_command(&format!(
            "chmod +x {script_path} {wrapper_path} \
            && tmux kill-session -t {session_name} 2>/dev/null; true \
            && tmux new-session -d -s {session_name} {wrapper_path}"
        ))
        .await?;

        tracing::info!(
            "tmux session '{}' started on '{}'. Log: {}",
            session_name,
            self.display(),
            log_path
        );
        Ok(())
    }

    /// Read the exit code written by `run_in_tmux`'s wrapper script.
    /// Returns `Ok(exit_code)` where a non-zero value means the script failed.
    #[allow(dead_code)]
    pub async fn tmux_exit_code(&self, session_name: &str) -> Result<i32> {
        let exit_path = format!("/tmp/hpcac_{}.exit", session_name);
        let raw = self
            .run_command(&format!("cat {exit_path} 2>/dev/null || echo 1"))
            .await
            .unwrap_or_else(|_| "1".to_string());
        Ok(raw.trim().parse::<i32>().unwrap_or(1))
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
    /// Transient SSH errors (e.g. banner-exchange timeout under heavy load) are
    /// treated as "still running" and logged as warnings rather than fatal failures.
    #[allow(dead_code)]
    pub async fn wait_for_tmux(&self, session_name: &str, poll_interval: Duration) -> Result<()> {
        let started = Instant::now();
        loop {
            match self.tmux_session_exists(session_name).await {
                Ok(false) => {
                    tracing::info!(
                        "tmux session '{}' on '{}' has finished",
                        session_name,
                        self.display()
                    );
                    return Ok(());
                }
                Ok(true) => {
                    let elapsed = started.elapsed().as_secs();
                    tracing::info!(
                        "tmux session '{}' on '{}' still running for {:02}:{:02}",
                        session_name,
                        self.display(),
                        elapsed / 3600,
                        (elapsed % 3600) / 60,
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Transient SSH error polling tmux session '{}' on '{}', will retry: {}",
                        session_name,
                        self.display(),
                        e
                    );
                }
            }
            sleep(poll_interval).await;
        }
    }

    /// Download the *contents* of a remote directory into a local directory using rsync.
    /// The remote directory itself is not created locally — only its contents are merged in.
    pub async fn download_dir(&self, remote_path: &str, local_path: &str) -> Result<()> {
        // Ensure trailing slash on remote so rsync copies contents, not the directory itself.
        let remote_src = format!(
            "{}@{}:{}/",
            self.username,
            self.ip,
            remote_path.trim_end_matches('/')
        );
        tracing::info!("Downloading '{}' -> '{}'", remote_src, local_path);

        let ssh_opts = format!(
            "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR",
            self.private_key_path
        );
        let output = Command::new("rsync")
            .args([
                "-az",
                "--no-relative",
                "-e", &ssh_opts,
                &remote_src,
                local_path,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "rsync failed downloading '{}': {}",
                remote_src,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    pub async fn upload_file_binary(&self, local_path: &str, remote_path: &str) -> Result<()> {
        tracing::info!(
            "Uploading '{}' -> '{}:{}'",
            local_path,
            self.display(),
            remote_path
        );
        let output = Command::new("scp")
            .args([
                "-i",
                &self.private_key_path,
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "LogLevel=ERROR",
                local_path,
                &format!("{}@{}:{}", self.username, self.ip, remote_path),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!(
                "scp failed uploading '{}' to '{}:{}': {}",
                local_path,
                self.display(),
                remote_path,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }
}
