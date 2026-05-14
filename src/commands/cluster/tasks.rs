use crate::database::models::{Cluster, ClusterState, TaskRun};
use crate::utils::{self, ssh::SshSession};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;

const MPI_HOSTFILE_PATH: &str = "/tmp/hpcac_hostfile";

#[derive(Debug, Deserialize, Serialize)]
struct UploadSpec {
    local: String,
    remote: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TasksYaml {
    cluster_id: String,
    results_remote_dir: String,
    results_local_dir: String,
    #[serde(default)]
    uploads: Vec<UploadSpec>,
    tasks: Vec<TaskYaml>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TaskYaml {
    name: String,
    mpi_slots_per_host: u32,
    repeat: u32,
    script: String,
}

pub async fn tasks(pool: &SqlitePool, yaml_file_path: &str) -> Result<()> {
    let path = Path::new(yaml_file_path);
    let yaml_str = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => anyhow::bail!("Failed to read file '{}': {}", yaml_file_path, e),
    };
    let tasks_yaml: TasksYaml = match serde_yaml::from_str(&yaml_str) {
        Ok(t) => t,
        Err(e) => anyhow::bail!("Failed to parse '{}': {}", yaml_file_path, e),
    };

    let cluster = match Cluster::fetch_by_id(pool, &tasks_yaml.cluster_id).await? {
        Some(c) => c,
        None => anyhow::bail!("Cluster '{}' not found", tasks_yaml.cluster_id),
    };

    if cluster.state != ClusterState::Running {
        anyhow::bail!(
            "Cluster '{}' is not running (current state: {})",
            cluster.id,
            cluster.state
        );
    }

    let mut nodes = cluster.get_nodes(pool).await?;
    if nodes.is_empty() {
        anyhow::bail!("Cluster '{}' has no nodes", cluster.id);
    }

    // Sort by private_ip for consistent ordering; node 0 = head node (mpirun is called here)
    nodes.sort_by(|a, b| a.private_ip.cmp(&b.private_ip));

    let head_ip = match &nodes[0].public_ip {
        Some(ip) => ip.clone(),
        None => anyhow::bail!("Head node has no public IP"),
    };

    let private_key_path = utils::expand_tilde(&cluster.private_ssh_key_path);
    let head_ssh = SshSession::for_aws(&head_ip, &private_key_path);

    let results_local_dir = utils::expand_tilde(&tasks_yaml.results_local_dir);
    let total_tasks = tasks_yaml.tasks.len();

    if !tasks_yaml.uploads.is_empty() {
        tracing::info!(
            "Uploading {} file(s) to cluster '{}' (head node: {})",
            tasks_yaml.uploads.len(),
            cluster.id,
            head_ip
        );
        for upload in &tasks_yaml.uploads {
            let local = utils::expand_tilde(&upload.local);
            let remote_dir = Path::new(&upload.remote)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if !remote_dir.is_empty() {
                head_ssh
                    .run_command(&format!(
                        "sudo mkdir -p {0} && sudo chown $(id -u):$(id -g) {0}",
                        remote_dir
                    ))
                    .await?;
            }
            head_ssh.upload_file_binary(&local, &upload.remote).await?;
        }
    }

    tracing::info!(
        "Running {} task(s) on cluster '{}' (head node: {})",
        total_tasks,
        cluster.id,
        head_ip
    );

    // Tracks the tmux session currently being waited on, so Ctrl+C can kill it.
    let active_session: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let result = tokio::select! {
        r = run_task_loop(
            pool, &head_ssh, &tasks_yaml, &nodes,
            &results_local_dir, total_tasks, &cluster.id,
            active_session.clone(),
        ) => r,
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Interrupted (Ctrl+C). Cleaning up...");
            let session = active_session.lock().unwrap().take();
            if let Some(session) = session {
                tracing::info!("Killing remote tmux session '{}'...", session);
                let _ = head_ssh
                    .run_command(&format!("tmux kill-session -t {} 2>/dev/null; true", session))
                    .await;
            }
            anyhow::bail!("Interrupted by user");
        }
    };

    result
}

async fn run_task_loop(
    pool: &SqlitePool,
    head_ssh: &SshSession,
    tasks_yaml: &TasksYaml,
    nodes: &[crate::database::models::Node],
    results_local_dir: &str,
    total_tasks: usize,
    cluster_id: &str,
    active_session: Arc<Mutex<Option<String>>>,
) -> Result<()> {
    for (task_idx, task) in tasks_yaml.tasks.iter().enumerate() {
        tracing::info!(
            "[{}/{}] Task '{}': {} repeat(s), {} slot(s)/host",
            task_idx + 1,
            total_tasks,
            task.name,
            task.repeat,
            task.mpi_slots_per_host
        );

        // Build MPI hostfile content for this task
        let hostfile_content: String = nodes
            .iter()
            .filter_map(|n| n.private_ip.as_deref())
            .map(|ip| format!("{} slots={}\n", ip, task.mpi_slots_per_host))
            .collect();

        for run in 1..=task.repeat {
            tracing::info!("  Run {}/{}", run, task.repeat);

            // Clean up leftover task scripts and logs from previous runs to avoid
            // filling /tmp and causing upload failures.
            head_ssh
                .run_command("rm -f /tmp/hpcac_*.sh /tmp/hpcac_*.log /tmp/hpcac_*.exit")
                .await?;

            head_ssh
                .upload_file(MPI_HOSTFILE_PATH, &hostfile_content)
                .await?;

            let session_name = format!("hpcac_{}_run{}", task.name.replace(['-', ' '], "_"), run);
            let wrapped_script = format!(
                "export HPCAC_RUN_INDEX={}\nexport HPCAC_HOSTFILE={}\n{}",
                run, MPI_HOSTFILE_PATH, task.script
            );

            let task_run = TaskRun::start(pool, cluster_id, &task.name, run as i64).await?;

            let launch_result = head_ssh.run_in_tmux(&session_name, &wrapped_script).await;
            if launch_result.is_err() {
                task_run.finish(pool, "failed").await?;
                launch_result?;
            }

            *active_session.lock().unwrap() = Some(session_name.clone());

            let wait_result = head_ssh
                .wait_for_tmux(&session_name, Duration::from_secs(300))
                .await;

            *active_session.lock().unwrap() = None;

            if let Err(e) = wait_result {
                task_run.finish(pool, "failed").await?;
                anyhow::bail!(
                    "Task '{}' run {}/{} failed (SSH error): {}",
                    task.name, run, task.repeat, e
                );
            }

            let exit_code = head_ssh.tmux_exit_code(&session_name).await?;
            if exit_code != 0 {
                task_run.finish(pool, "failed").await?;
                anyhow::bail!(
                    "Task '{}' run {}/{} failed with exit code {}. Check log: /tmp/hpcac_{}.log",
                    task.name, run, task.repeat, exit_code, session_name
                );
            }

            task_run.finish(pool, "success").await?;
        }

        // Collect results after all repeats of this task complete.
        // Download contents of remote results dir directly into results_local_dir
        // (scripts already organize output by task name inside that dir).
        // Then clear the remote so the next task starts with a clean slate.
        fs::create_dir_all(results_local_dir)?;
        tracing::info!(
            "  Collecting results: '{}' -> '{}'",
            tasks_yaml.results_remote_dir,
            results_local_dir
        );
        head_ssh
            .run_command(&format!("mkdir -p {}", tasks_yaml.results_remote_dir))
            .await?;
        head_ssh
            .download_dir(&tasks_yaml.results_remote_dir, results_local_dir)
            .await?;
        head_ssh
            .run_command(&format!("rm -rf {}/*", tasks_yaml.results_remote_dir))
            .await?;

        tracing::info!("  Task '{}' complete.", task.name);
    }

    tracing::info!("All tasks complete.");
    Ok(())
}
