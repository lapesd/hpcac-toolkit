use crate::commands::utils;
use crate::database::models::{
    Cluster, InstanceType, Node, Provider, ProviderConfig, ShellCommand,
};
use crate::integrations::{cloud_interface::CloudInfoProvider, providers::aws::AwsInterface};
use chrono::Utc;
use inquire::{Confirm, Select};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use tracing::{error, info};

#[derive(Debug, Deserialize, Serialize)]
struct ClusterYaml {
    display_name: String,
    provider_id: Option<String>,
    provider_config_id: Option<i64>,
    private_ssh_key_path: String,
    public_ssh_key_path: String,
    region: String,
    availability_zone: String,
    nodes: Vec<NodeYaml>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeYaml {
    instance_type: String,
    allocation_mode: Option<String>,
    burstable_mode: Option<String>,
    image_id: String,
    init_commands: Option<Vec<String>>,
}

pub async fn create(
    pool: &SqlitePool,
    yaml_file_path: &str,
    skip_confirmation: bool,
) -> anyhow::Result<()> {
    // Open and parse the cluster blueprint yaml file
    let path = Path::new(yaml_file_path);
    let cluster_yaml_str: String = match fs::read_to_string(path) {
        Ok(result) => {
            info!("Successfully read file: '{}'", yaml_file_path);
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            anyhow::bail!("Failed to read file '{}'", yaml_file_path)
        }
    };

    let cluster_yaml: ClusterYaml = match serde_yaml::from_str(&cluster_yaml_str) {
        Ok(result) => {
            info!("Parsed cluster blueprint yaml file successfully");
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            anyhow::bail!("Failed to parse yaml file: '{}'", yaml_file_path)
        }
    };

    // Validate provided SSH key pair
    let public_key_path_string = utils::expand_tilde(&cluster_yaml.public_ssh_key_path);
    let public_key_path = Path::new(&public_key_path_string);
    let _public_ssh_key = match fs::read_to_string(public_key_path) {
        Ok(result) => {
            info!("Successfully read file: '{}'", &public_key_path_string);
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            anyhow::bail!("Failed to read file: '{}'", &public_key_path_string)
        }
    };
    let private_key_path_string = utils::expand_tilde(&cluster_yaml.private_ssh_key_path);
    let private_key_path = Path::new(&private_key_path_string);
    let _private_ssh_key = match fs::read_to_string(private_key_path) {
        Ok(result) => {
            info!("Successfully read file: '{}'", &private_key_path_string);
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            anyhow::bail!("Failed to read file: '{}'", &private_key_path_string)
        }
    };

    // Validate provider_config if provided, else prompt user for selection
    let provider_config = match cluster_yaml.provider_config_id {
        Some(config_id) => {
            let config_query = ProviderConfig::fetch_by_id(pool, config_id).await?;
            match config_query {
                Some(result) => {
                    info!("Provider Configuration: '{}' found", config_id);
                    result
                }
                None => {
                    anyhow::bail!("Provider Configuration: '{}' not found", config_id)
                }
            }
        }
        None => {
            let provider = match &cluster_yaml.provider_id {
                Some(provider_id) => {
                    let provider_query = Provider::fetch_by_id(pool, provider_id.clone()).await?;
                    match provider_query {
                        Some(result) => {
                            info!("Provider: '{}' found", provider_id);
                            result
                        }
                        None => {
                            anyhow::bail!("Provider '{}' not found", provider_id)
                        }
                    }
                }
                None => {
                    anyhow::bail!(
                        "Neither 'provider_id' or 'provider_configuration_id' are defined in '{}'",
                        yaml_file_path
                    )
                }
            };

            let mut configs = ProviderConfig::fetch_all_by_provider(pool, &provider.id).await?;
            if configs.is_empty() {
                anyhow::bail!(
                    "No Provider Configurations found. Use 'provider-config create' to setup one"
                )
            } else if configs.len() == 1 {
                // Use the only config available
                configs.swap_remove(0)
            } else {
                let config_options: Vec<&str> =
                    configs.iter().map(|p| p.display_name.as_str()).collect();
                let selected_config =
                    match Select::new("Select a provider configuration:\n", config_options)
                        .without_filtering()
                        .prompt()
                    {
                        Ok(selection) => selection,
                        Err(e) => {
                            error!("{}", e.to_string());
                            anyhow::bail!("Failed to get user selection")
                        }
                    };

                let selected_index = configs
                    .iter()
                    .position(|p| p.display_name == selected_config)
                    .unwrap();

                configs.swap_remove(selected_index)
            }
        }
    };

    // Get cloud interface
    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => {
            anyhow::bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    println!("Validating cloud provider connection and cluster node data...");

    // Check region
    let regions_tracker = utils::ProgressTracker::new(1, Some("region discovery"));
    let regions = cloud_interface.fetch_regions(&regions_tracker).await?;
    regions_tracker.finish_with_message(&format!(
        "Region discovery complete: found {} regions in {}",
        regions.len(),
        provider_id
    ));
    let region = cluster_yaml.region.clone();
    if !regions.contains(&region) {
        anyhow::bail!(
            "Region '{}' is not available. Possible options: {:?}",
            region,
            regions
        )
    }

    // Check availability_zone
    let zones_tracker = utils::ProgressTracker::new(1, Some("availability zones discovery"));
    let availability_zones = cloud_interface.fetch_zones(&region, &zones_tracker).await?;
    zones_tracker.finish_with_message(&format!(
        "Availability zone discovery complete: found {} zones in region {}",
        availability_zones.len(),
        &region
    ));
    let availability_zone = cluster_yaml.availability_zone.clone();
    if !availability_zones.contains(&availability_zone) {
        anyhow::bail!(
            "Availability zone '{}' is not available. Possible options: {:?}",
            availability_zone,
            availability_zones
        )
    }

    // Validate node data
    let new_cluster_id = utils::generate_id();
    let mut nodes_to_insert: Vec<Node> = vec![];
    let mut commands_to_insert: Vec<ShellCommand> = vec![];
    let node_count = cluster_yaml.nodes.len() as u64;
    let nodes_tracker = utils::ProgressTracker::new(node_count, Some("nodes validation"));
    for (i, node_definition) in cluster_yaml.nodes.iter().enumerate() {
        let instance_type_name = node_definition.instance_type.clone();

        // Validate instance_type
        let instance_type_details = match InstanceType::fetch_by_name_and_region(
            pool,
            &instance_type_name,
            &region,
        )
        .await
        {
            Ok(Some(details)) => details,
            Ok(None) => {
                anyhow::bail!(
                    "Instance type '{}' is unavailable in provider '{}' at region '{}'.\n\
                    Are the instance_types loaded? Use 'instance_type list' to check loaded data",
                    &instance_type_name,
                    &provider_id,
                    &region
                )
            }
            Err(e) => {
                error!("{}", e.to_string());
                anyhow::bail!("DB Operation Failure")
            }
        };

        // Validate allocation_mode
        let allocation_mode = match &node_definition.allocation_mode {
            Some(mode) => match mode.to_lowercase().as_str() {
                "spot" => match instance_type_details.supports_spot {
                    true => mode.to_string(),
                    false => {
                        anyhow::bail!(
                            "Failed validating allocation_mode for node '{}': 'spot' mode not \
                            available for instance_type '{}' in region '{}'",
                            i,
                            &instance_type_name,
                            &region
                        )
                    }
                },
                "on-demand" | "on_demand" => "on-demand".to_string(),
                invalid_mode => {
                    anyhow::bail!(
                        "Failed validating allocation_mode for node '{}': '{}' is not a valid \
                        allocation_mode",
                        i,
                        invalid_mode,
                    )
                }
            },
            None => "on-demand".to_string(), // Default when not specified
        };

        // Validate burstable_mode
        let burstable_mode = match &node_definition.burstable_mode {
            Some(mode) => match instance_type_details.is_burstable {
                true => {
                    // TODO: validate if the burstable_mode string matches a supported burstable
                    // performance mode.
                    Some(mode)
                }
                false => {
                    anyhow::bail!(
                        "Failed validating burstable_mode for instance '{}': 'burstable' mode \
                        not available for instance_type '{}' in region '{}'",
                        i,
                        &instance_type_name,
                        &region
                    )
                }
            },
            None => None,
        };

        // Validate image_id
        let image_id = node_definition.image_id.clone();
        cloud_interface
            .fetch_machine_image(&region, &image_id)
            .await?;

        // Push shell commands to be inserted
        let new_node_id = utils::generate_id();
        if let Some(init_commands) = &node_definition.init_commands {
            for (i, command) in init_commands.iter().enumerate() {
                commands_to_insert.push(ShellCommand {
                    id: 0, // placeholder
                    node_id: new_node_id.clone(),
                    ordering: (i + 1) as i64,
                    script: command.clone(),
                    execution_time: None,
                    result: None,
                    status: "NOT_EXECUTED".to_string(),
                    triggered_at: None,
                });
            }
        }

        nodes_to_insert.push(Node {
            id: new_node_id,
            cluster_id: new_cluster_id.clone(),
            status: "PLANNED".to_string(),
            instance_type: instance_type_name,
            allocation_mode,
            burstable_mode: burstable_mode.cloned(),
            image_id,
            private_ip: None,
            public_ip: None,
        });
        nodes_tracker.inc();
    }
    nodes_tracker.finish_with_message(&format!("Validated {} nodes", node_count));

    println!("\n\n=== New Cluster Blueprint Information ===");
    println!("{:<20}: {}", "Provider", provider_config.provider_id);
    println!("{:<20}: {}", "Region", region);
    println!("{:<20}: {}", "Availability Zone", availability_zone);
    println!(
        "{:<20}: {}",
        "Provider Config", provider_config.display_name
    );
    println!("{:<20}: {}\n", "Node Count", cluster_yaml.nodes.len());

    println!("Node Definitions:");
    for (i, node) in cluster_yaml.nodes.iter().enumerate() {
        println!("  Node {}:", i + 1);
        println!("    Instance Type   : {}", node.instance_type);
        println!("    Image ID        : {}", node.image_id);
        println!(
            "    Allocation Mode : {}",
            node.allocation_mode.as_deref().unwrap_or("on-demand")
        );
        println!(
            "    Burstable Mode  : {}",
            node.burstable_mode.as_deref().unwrap_or("N/A")
        );
        if let Some(cmds) = &node.init_commands {
            println!("    Init Commands   :");
            for cmd in cmds {
                println!("      - {}", cmd);
            }
        }
        println!();
    }

    if !skip_confirmation {
        let confirm = Confirm::new("Do you want to proceed with storing this cluster blueprint?")
            .with_default(true)
            .prompt();
        match confirm {
            Ok(true) => info!("Confirmed! Proceeding with cluster blueprint creation..."),
            Ok(false) => {
                println!("Operation cancelled by user");
                return Ok(());
            }
            Err(e) => {
                error!("{}", e.to_string());
                anyhow::bail!("Error processing user response")
            }
        }
    } else {
        info!("Automatic confirmation with -y flag. Proceeding...");
    }

    let cluster_name = cluster_yaml.display_name.clone();
    let cluster = Cluster {
        id: new_cluster_id.clone(),
        display_name: cluster_name.clone(),
        provider_id,
        provider_config_id: provider_config.id,
        public_ssh_key_path: public_key_path_string,
        private_ssh_key_path: private_key_path_string,
        region,
        created_at: Utc::now().naive_utc(),
        spawned_at: None,
    };
    cluster
        .insert(pool, nodes_to_insert, commands_to_insert)
        .await?;

    println!(
        "New Cluster '{}' created successfully! To spawn it, use: 'cluster spawn'",
        cluster_name
    );
    Ok(())
}
