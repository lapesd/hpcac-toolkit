use crate::database::models::{ Cluster, ProviderConfig };
use crate::integrations::providers::aws::AwsInterface;
use crate::database::models::{ ClusterState, InstanceType };
use crate::utils;

use std::time;
use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use tracing::{info, error};
use aws_sdk_ec2::types::Filter;
use chrono::Local;
use std::io::Write;

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
    info!("Parsing contents of `tasks_config.yaml` file...");

    // Load and parse the task YAML
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
    // get cluster and nodes
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

    // Get cloud interface
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
    if !(utils::user_confirmation(
        skip_confirmation,
        "Run this tasks on the cluster?",
    )?) {
        return Ok(());
    }
    println!();

    // Get context and task_runner_instance_id
    let context = cloud_interface.create_cluster_context(&cluster)?;
    let task_runner_instance_name = context.ec2_instance_name(0);
    let filter = Filter::builder()
        .name("tag:Name")
        .values(&task_runner_instance_name)
        .build();
    let resp = context.ec2_client.describe_instances()
        .filters(filter)
        .send()
        .await?;
    let ec2_id = resp.reservations()
        .iter()
        .flat_map(|r| r.instances())
        .find_map(|i| i.instance_id().map(|id| id.to_string()));
    let task_runner_instance_ec2_id = match ec2_id {
        Some(id) => id,
        None     => bail!("Unable to retrieve ec2 instance id."),
    };

    // Report result string
    let mut report_str = String::new();
    report_str.push_str(&format!("-=-=-=-=-=-=-=-= CLUSTER DETAILS =-=-=-=-=-=-=-=-\n"));
    report_str.push_str(&format!("{:<35}: {}\n", "Cluster Name", cluster.display_name));
    report_str.push_str(&format!("{:<35}: {}\n", "Provider", cluster.provider_id));
    report_str.push_str(&format!("{:<35}: {}\n", "Region", cluster.region));
    report_str.push_str(&format!("{:<35}: {}\n", "Availability Zone", cluster.availability_zone));
    report_str.push_str(&format!("{:<35}: {}\n", "Use Node Affinity", cluster.use_node_affinity));
    report_str.push_str(&format!("{:<35}: {}\n", "Use Elastic Fabric Adapters (EFAs)", cluster.use_elastic_fabric_adapters));
    report_str.push_str(&format!("{:<35}: {}\n", "Use Elastic File System (EFS)", cluster.use_elastic_file_system));
    report_str.push_str(&format!("{:<35}: {}\n", "On Instance Creation Failure", cluster.on_instance_creation_failure.clone().unwrap().to_string()));
    report_str.push_str(&format!("{:<35}: {}\n", "Provider Config", provider_config.display_name));
    report_str.push_str(&format!("{:<35}: {}\n\n", "Node Count", nodes.len()));

    report_str.push_str(&format!("Node Details:\n"));
    for (i, node) in nodes.iter().enumerate() {
        let instance_type_name = &node.instance_type;
        let instance_details = InstanceType::fetch_by_name_and_region(pool, instance_type_name, &cluster.region).await?.unwrap();
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

        report_str.push_str(&format!("  Node {}:\n", i + 1));
        report_str.push_str(&format!("    Instance Type   : {}\n", node.instance_type));
        report_str.push_str(&format!("    Processor       : {}\n", processor_info));
        report_str.push_str(&format!("    vCPUs:          : {}\n", instance_details.vcpus));
        report_str.push_str(&format!("    GPUs:           : {}\n", gpu_info));
        report_str.push_str(&format!("    Image ID        : {}\n", node.image_id));
        report_str.push_str(&format!("    Allocation Mode : {}\n", node.allocation_mode));
        report_str.push_str(&format!("    Burstable Mode  : {}\n",
            node.burstable_mode.as_deref().unwrap_or("N/A")
        ));
    }
    report_str.push_str(&format!("-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-\n\n"));

    // Running tasks
    let steps: usize = tasks_yaml.tasks.iter()
        .fold(0, |acc, task| acc + task.setup_commands.len() + task.run_commands.len());

    let multi = utils::ProgressTracker::create_multi();
    let main_progress = utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some("Initializing..."));
    let operation_spinner = utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

    info!("Starting Task loop...");
    for task in tasks_yaml.tasks.iter() {
        report_str.push_str(&format!("===> Task: '{}'\n", task.task_tag));

        let running_task_message = format!("Running task '{}' setup commands...", task.task_tag);
        info!(running_task_message);
        main_progress.update_message(&running_task_message);

        let setup_commands_start = time::Instant::now();
        for command in task.setup_commands.iter() {
            operation_spinner.update_message(&format!("Executing command: '{}'", command));
            report_str.push_str(&format!("$ {}\n", command));
            
            match cloud_interface.send_and_wait_for_ssm_command(&context, &task_runner_instance_ec2_id, command.clone()).await {
                Ok(out) => report_str.push_str(&format!("{}\n", out)),
                Err(e)  => report_str.push_str(&format!("error: {}\n\n", e)),
            }

            main_progress.inc(1);
        }
        let setup_commands_elapsed_ms = setup_commands_start.elapsed().as_millis();
        let setup_commands_elapsed_sec = setup_commands_elapsed_ms as f64 / 1000.0;

        let running_task_message = format!("Running task '{}' run_commands...", task.task_tag);
        info!(running_task_message);
        main_progress.update_message(&running_task_message);

        let run_commands_start = time::Instant::now();
        for command in task.run_commands.iter() {
            operation_spinner.update_message(&format!("Executing command: '{}'", command));
            report_str.push_str(&format!("$ {}\n", command));
            
            match cloud_interface.send_and_wait_for_ssm_command(&context, &task_runner_instance_ec2_id, command.clone()).await {
                Ok(out) => report_str.push_str(&format!("{}\n", out)),
                Err(e)  => report_str.push_str(&format!("{}\n\n", e)),
            }

            main_progress.inc(1);
        }

        let run_commands_elapsed_ms = run_commands_start.elapsed().as_millis();
        let run_commands_elapsed_sec = run_commands_elapsed_ms as f64 / 1000.0;
        let exec_time = setup_commands_elapsed_sec + run_commands_elapsed_sec;

        report_str.push_str(
            &format!("===== End of Task '{}' - setup time: {:.3} s - run time: {:.3} s - total: {} s =====\n\n",
                task.task_tag, setup_commands_elapsed_sec, run_commands_elapsed_sec, exec_time)
        );
    }

    operation_spinner.finish_with_message("All commands of all tasks completed!");
    main_progress.finish_with_message("All tasks completed!");
    info!("All tasks completed!");

    // save report
    let mut created_dir = true;
    let report_path = format!("results/cluster_{}", cluster_id);
    if let Err(e) = fs::create_dir_all(&report_path) {
        println!("Failed to create directory for result report: {}", e);
        created_dir = false;
    };

    if created_dir {
        let timestamp = Local::now().format("%Y-%m-%dT%H:%M:%S");
        let filename = format!("{}.txt", timestamp);
        match fs::File::create(&format!("{}/{}", report_path, filename)) {
            Err(e) => println!("Failed to create report file: {}", e),
            Ok(mut file) => {
                writeln!(file, "{}", report_str)?;
                println!("Result report saved at '{}/{}'", report_path, filename);
            },
        };
    }

    Ok(())
}

