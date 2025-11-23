use crate::database::models::{Cluster, ClusterState, InstanceType, ProviderConfig};
use crate::integrations::providers::aws::AwsInterface;
use crate::utils;

use anyhow::{bail, Result};
use aws_sdk_ec2::types::Filter;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{error, info};

#[derive(Debug, Deserialize, Serialize)]
struct TasksYaml {
    tasks: Vec<Task>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Task {
    task_tag: String,
    setup_commands: Vec<String>,
    run_commands: Vec<String>,
}

pub async fn run_task(
    pool: &SqlitePool,
    yaml_file_path: &str,
    cluster_id: &str,
    skip_confirmation: bool,
) -> Result<()> {
    info!("Invoked `run_tasks` command...");

    // Prepare report file
    let mut report_dir = PathBuf::from("results");
    report_dir.push(format!("cluster_{}", cluster_id));

    if let Err(e) = fs::create_dir_all(&report_dir) {
        error!("Failed to create directory for result report: {}", e);
        bail!("FileSystem error: {}", e);
    }

    let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S");
    let filename = format!("{}.txt", timestamp);
    let report_path = report_dir.join(&filename);

    // Open file in Append/Create mode
    let mut report_file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&report_path)
    {
        Ok(f) => {
            info!("Result report will be streamed to '{:?}'", report_path);
            f
        }
        Err(e) => {
            error!("Failed to create report file: {}", e);
            bail!("FileSystem error: {}", e);
        }
    };

    // Helper closure to write to file and handle errors cleanly
    macro_rules! log_report {
        ($($arg:tt)*) => ({
            let text = format!($($arg)*);
            // print!("{}", text);
            if let Err(e) = report_file.write_all(text.as_bytes()) {
                error!("Failed to write to report file: {}", e);
            }
            if let Err(e) = report_file.flush() {
                error!("Failed to flush report file: {}", e);
            }
        })
    }

    info!("Parsing contents of `tasks_config.yaml` file...");
    let path = Path::new(yaml_file_path);
    let tasks_yaml_str: String = match fs::read_to_string(path) {
        Ok(result) => {
            info!("Successfully read file: '{}'", yaml_file_path);
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            bail!("Failed to read file '{}'", yaml_file_path)
        }
    };

    let tasks_yaml: TasksYaml = match serde_yaml::from_str(&tasks_yaml_str) {
        Ok(result) => {
            info!("Parsed tasks yaml file successfully");
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            bail!(
                "Failed to parse yaml file: '{}': {:?}",
                yaml_file_path,
                e.to_string()
            )
        }
    };

    info!("fetching Clusters (id='{}')", cluster_id);
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            println!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };
    let nodes = cluster.get_nodes(pool).await?;

    if cluster.state != ClusterState::Running {
        println!("Cluster with id '{}' was not spawned.", cluster_id);
        return Ok(());
    }
    info!("Found online Cluster (id='{}')!", cluster_id);

    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => {
                error!("Missing ProviderConfig '{}'", cluster.provider_config_id);
                bail!("Data Consistency Failure");
            }
        };
    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => {
            bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    // Confirm with user
    println!("Tasks:");
    for task in tasks_yaml.tasks.iter() {
        println!(" - name: {}", task.task_tag);
        println!("   setup_commands:");
        for command in task.setup_commands.iter() {
            println!("     - {}", command);
        }
        println!("   run_commands:");
        for command in task.run_commands.iter() {
            println!("     - {}", command);
        }
        println!();
    }
    if !(utils::user_confirmation(skip_confirmation, "Run this tasks on the cluster?")?) {
        return Ok(());
    }
    println!();

    // Get context and task_runner_instance_id
    let context = cloud_interface.create_cluster_context(&cluster)?;
    let task_runner_instance_name = context.ec2_instance_name(0);
    
    // Filter by Name
    let name_filter = Filter::builder()
        .name("tag:Name")
        .values(&task_runner_instance_name)
        .build();

    // Filter by State
    let state_filter = Filter::builder()
        .name("instance-state-name")
        .values("running")
        .build();

    let resp = context
        .ec2_client
        .describe_instances()
        .filters(name_filter)
        .filters(state_filter)
        .send()
        .await?;

    let ec2_id = resp
        .reservations()
        .iter()
        .flat_map(|r| r.instances())
        .find_map(|i| i.instance_id().map(|id| id.to_string()));

    let task_runner_instance_ec2_id = match ec2_id {
        Some(id) => id,
        None => bail!("Unable to retrieve ec2 instance id."),
    };

    info!("Checking SSM Agent status for instance '{}'...", task_runner_instance_ec2_id);
    println!("Waiting for node to be ready for commands (SSM Agent)...");
    
    cloud_interface.wait_for_ssm_agent_ready(
        &context, 
        &task_runner_instance_ec2_id, 
        Duration::from_secs(300) // Wait up to 5 minutes
    ).await?;
    
    info!("SSM Agent is ready!");

    // Write cluster details to file
    log_report!("-=-=-=-=-=-=-=-= CLUSTER DETAILS =-=-=-=-=-=-=-=-\n");
    log_report!("{:<35}: {}\n", "Cluster Name", cluster.display_name);
    log_report!("{:<35}: {}\n", "Provider", cluster.provider_id);
    log_report!("{:<35}: {}\n", "Region", cluster.region);
    log_report!(
        "{:<35}: {}\n",
        "Availability Zone",
        cluster.availability_zone
    );
    log_report!("{:<35}: {}\n", "Use Node Affinity", cluster.use_node_affinity);
    log_report!(
        "{:<35}: {}\n",
        "Use Elastic Fabric Adapters (EFAs)",
        cluster.use_elastic_fabric_adapters
    );
    log_report!(
        "{:<35}: {}\n",
        "Use Elastic File System (EFS)",
        cluster.use_elastic_file_system
    );
    log_report!(
        "{:<35}: {}\n",
        "On Instance Creation Failure",
        cluster
            .on_instance_creation_failure
            .clone()
            .unwrap()
            .to_string()
    );
    log_report!(
        "{:<35}: {}\n",
        "Provider Config",
        provider_config.display_name
    );
    log_report!("{:<35}: {}\n\n", "Node Count", nodes.len());

    log_report!("Node Details:\n");
    for (i, node) in nodes.iter().enumerate() {
        let instance_type_name = &node.instance_type;
        let instance_details = InstanceType::fetch_by_name_and_region(
            pool,
            instance_type_name,
            &cluster.region,
        )
        .await?
        .unwrap();
        let processor_info = match &instance_details.core_count {
            Some(cores) => {
                format!(
                    "{}-Core {} {}",
                    cores, instance_details.cpu_architecture, instance_details.cpu_type
                )
            }
            None => {
                format!(
                    "{} {}",
                    instance_details.cpu_architecture, instance_details.cpu_type
                )
            }
        };

        let gpu_info = match instance_details.gpu_type {
            Some(gpu) => {
                format!("{}x {}", instance_details.gpu_count, gpu)
            }
            None => "N/A".to_string(),
        };

        log_report!("  Node {}:\n", i + 1);
        log_report!("    Instance Type   : {}\n", node.instance_type);
        log_report!("    Processor       : {}\n", processor_info);
        log_report!("    vCPUs:          : {}\n", instance_details.vcpus);
        log_report!("    GPUs:           : {}\n", gpu_info);
        log_report!("    Image ID        : {}\n", node.image_id);
        log_report!("    Allocation Mode : {}\n", node.allocation_mode);
        log_report!(
            "    Burstable Mode  : {}\n",
            node.burstable_mode.as_deref().unwrap_or("N/A")
        );
    }
    log_report!("-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n\n");

    // Running tasks
    let steps: usize = tasks_yaml.tasks.iter().fold(0, |acc, task| {
        acc + task.setup_commands.len() + task.run_commands.len()
    });

    let multi = utils::ProgressTracker::create_multi();
    let main_progress =
        utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some("Initializing..."));
    let operation_spinner = utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

    info!("Starting Task loop...");
    for task in tasks_yaml.tasks.iter() {
        log_report!("===> Task: '{}'\n", task.task_tag);

        let running_task_message = format!("Running task '{}' setup commands...", task.task_tag);
        info!(running_task_message);
        main_progress.update_message(&running_task_message);

        let setup_commands_start = Instant::now();
        for command in task.setup_commands.iter() {
            operation_spinner.update_message(&format!("Executing command: '{}'", command));
            log_report!("$ {}\n", command);

            let result = async {
                // Create Command
                let cmd_id = cloud_interface
                    .create_ssm_command(
                        &context,
                        &task_runner_instance_ec2_id,
                        command.clone(),
                    )
                    .await?;

                // Poll until completion
                cloud_interface
                    .poll_ssm_command_until_completion(
                        &context,
                        &cmd_id,
                        &task_runner_instance_ec2_id,
                        Duration::from_secs(3600), // 1 hour timeout
                        Duration::from_secs(2),
                    )
                    .await
            }
            .await;

            match result {
                Ok(out) => log_report!("{}\n", out),
                Err(e) => log_report!("error: {}\n\n", e),
            }

            main_progress.inc(1);
        }
        let setup_commands_elapsed_sec = setup_commands_start.elapsed().as_secs_f64();

        let running_task_message = format!("Running task '{}' run_commands...", task.task_tag);
        info!(running_task_message);
        main_progress.update_message(&running_task_message);

        let run_commands_start = Instant::now();
        for command in task.run_commands.iter() {
            operation_spinner.update_message(&format!("Executing command: '{}'", command));
            log_report!("$ {}\n", command);

            let result = async {
                let cmd_id = cloud_interface
                    .create_ssm_command(
                        &context,
                        &task_runner_instance_ec2_id,
                        command.clone(),
                    )
                    .await?;

                cloud_interface
                    .poll_ssm_command_until_completion(
                        &context,
                        &cmd_id,
                        &task_runner_instance_ec2_id,
                        Duration::from_secs(3600),
                        Duration::from_secs(2),
                    )
                    .await
            }
            .await;

            match result {
                Ok(out) => log_report!("{}\n", out),
                Err(e) => log_report!("{}\n\n", e),
            }

            main_progress.inc(1);
        }

        let run_commands_elapsed_sec = run_commands_start.elapsed().as_secs_f64();
        let exec_time = setup_commands_elapsed_sec + run_commands_elapsed_sec;

        log_report!(
            "===== End of Task '{}' - setup time: {:.3} s - run time: {:.3} s - total: {:.3} s =====\n\n",
            task.task_tag,
            setup_commands_elapsed_sec,
            run_commands_elapsed_sec,
            exec_time
        );
    }

    operation_spinner.finish_with_message("All commands of all tasks completed!");
    main_progress.finish_with_message("All tasks completed!");
    info!("All tasks completed!");

    info!("Result report saved at '{:?}'", report_path);

    Ok(())
}
