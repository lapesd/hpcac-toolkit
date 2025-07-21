use crate::database::models::{ Cluster, ProviderConfig };
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::database::models::ClusterState;
use crate::utils;

use std::{thread, time};

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use tracing::{info, error};

#[derive(Debug, Deserialize, Serialize)]
struct TasksYaml {
    tasks: Vec<Task>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Task {
    task_name: String,
    commands: Vec<String>,
}

pub async fn run_task(
    pool: &SqlitePool,
    yaml_file_path: &str,
    cluster_id: &str,
    skip_confirmation: bool,
) -> Result<()> {
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

    // get cluster and nodes
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            println!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    if cluster.state != ClusterState::Running {
        println!("Cluster '{}' was not spawned.", cluster_id);
        return Ok(());
    }

    let nodes = cluster.get_nodes(pool).await?;

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
        println!(" - name: {}", task.task_name);
        println!(" - commands:");
        for command in task.commands.iter() {
            println!("     - {}", command);
        }
        println!("");
    }
    if !(utils::user_confirmation(
        skip_confirmation,
        "Run this tasks on the cluster?",
    )?) {
        return Ok(());
    }
    println!("");

    // Get context and master node ec2 id
    let context = cloud_interface.create_cluster_context(&cluster)?;
    //let ec2_instance_id = &context.ec2_instance_ids.get(&0).unwrap();
    let ec2_instance_id = "i-0f3fe6cf06e65b9d3";

    // Running tasks
    let steps: usize = tasks_yaml.tasks.iter().map(|task| task.commands.len()).sum();
    let mut output = String::new();
    let multi = utils::ProgressTracker::create_multi();
    let main_progress = utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some("Initializing..."));
    let operation_spinner = utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");
    for task in tasks_yaml.tasks.iter() {
        output.push_str(&format!(" - task {}\n", task.task_name));

        let running_task_message = format!("Running task '{}'...", task.task_name);
        info!(running_task_message);
        main_progress.update_message(&running_task_message);

        for command in task.commands.iter() {
            operation_spinner.update_message(&format!("Executing command: '{}'", command));
            output.push_str(&format!("$ {}\n", command));
            
            match cloud_interface.send_and_wait_for_ssm_command(&context, ec2_instance_id, command.clone()).await {
                Ok(out) => output.push_str(&format!("{}\n", out)),
                Err(e)     => output.push_str(&format!("error: {}\n", e)),
            }

            //thread::sleep(time::Duration::from_secs(5));
            main_progress.inc(1);
        }
    }

    operation_spinner.finish_with_message("All commands of all tasks executed successfully!");
    main_progress.finish_with_message("All tasks executed successfully!");

    println!("{}", output);
    Ok(())
}
