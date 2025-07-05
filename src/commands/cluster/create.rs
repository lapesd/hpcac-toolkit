use crate::database::models::{
    Cluster, ClusterState, InstanceType, Node, Provider, ProviderConfig, ShellCommand, InstanceCreationFailurePolicy
};
use crate::integrations::{cloud_interface::CloudInfoProvider, providers::aws::AwsInterface};
use crate::utils;

use anyhow::{Result, bail};
use chrono::Utc;
use inquire::Select;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use tracing::{error, info};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize)]
struct ClusterYaml {
    id: Option<String>,
    display_name: String,
    provider_id: Option<String>,
    provider_config_id: Option<i64>,
    private_ssh_key_path: String,
    public_ssh_key_path: String,
    region: String,
    availability_zone: String,
    use_node_affinity: bool,
    use_elastic_fabric_adapters: bool,
    use_elastic_file_system: bool,
    on_instance_creation_failure: String,
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
) -> Result<()> {
    let path = Path::new(yaml_file_path);
    let cluster_yaml_str: String = match fs::read_to_string(path) {
        Ok(result) => {
            info!("Successfully read file: '{}'", yaml_file_path);
            result
        }
        Err(e) => {
            error!("{}", e.to_string());
            bail!("Failed to read file '{}'", yaml_file_path)
        }
    };

    let cluster_yaml: ClusterYaml = match serde_yaml::from_str(&cluster_yaml_str) {
        Ok(result) => {
            info!("Parsed cluster yaml file successfully");
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
            bail!("Failed to read file: '{}'", &public_key_path_string)
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
            bail!("Failed to read file: '{}'", &private_key_path_string)
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
                    bail!("Provider Configuration: '{}' not found", config_id)
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
                            bail!("Provider '{}' not found", provider_id)
                        }
                    }
                }
                None => {
                    bail!(
                        "Neither 'provider_id' or 'provider_configuration_id' are defined in '{}'",
                        yaml_file_path
                    )
                }
            };

            let mut configs = ProviderConfig::fetch_all_by_provider(pool, &provider.id).await?;
            if configs.is_empty() {
                bail!("No Provider Configurations found. Use 'provider-config create' to setup one")
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
                            bail!("Failed to get user selection")
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
        "aws" => AwsInterface { config_vars, db_pool: Arc::new(pool.clone())},
        _ => {
            bail!("Provider '{}' is currently not supported.", &provider_id)
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
        bail!(
            "Region '{}' is not available. Possible options: {:?}",
            region,
            regions
        )
    }

    // Check availability_zone, if present
    let zones_tracker = utils::ProgressTracker::new(1, Some("zones discovery"));
    let zones = cloud_interface.fetch_zones(&region, &zones_tracker).await?;
    zones_tracker.finish_with_message(&format!(
        "Zone discovery complete: found {} zones in {}",
        zones.len(),
        region
    ));
    let zone = cluster_yaml.availability_zone.clone();
    if !zones.contains(&zone) {
        bail!(
            "Availability Zone '{}' is not available. Possible options: {:?}",
            zone,
            zones
        )
    }

    // get the on_instance_creation_failure 
    let failure_policy = match cluster_yaml.on_instance_creation_failure.to_lowercase().as_str() {
        "migrate" => InstanceCreationFailurePolicy::Migrate,
        "cancel"  => InstanceCreationFailurePolicy::Cancel, // Default to Cancel for any other value
        other     => bail!("Invalid value for on_instance_creation_failure: '{}'. Expected 'migrate' or 'cancel'", other),
    };

    // Validate node data
    let new_cluster_id = match cluster_yaml.id {
        Some(id) => id,
        None => utils::generate_id(),
    };
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
                bail!(
                    "Instance type '{}' is unavailable in provider '{}' at region '{}'.\n\
                    Are the instance_types loaded? Use 'instance_type list' to check loaded data",
                    &instance_type_name,
                    &provider_id,
                    &region
                )
            }
            Err(e) => {
                error!("{}", e.to_string());
                bail!("DB Operation Failure")
            }
        };

        // Validate allocation_mode
        let allocation_mode = match &node_definition.allocation_mode {
            Some(mode) => match mode.to_lowercase().as_str() {
                "spot" => match instance_type_details.supports_spot {
                    true => mode.to_string(),
                    false => {
                        bail!(
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
                    bail!(
                        "Failed validating allocation_mode for node '{}': '{}' is not a valid \
                        allocation_mode",
                        i,
                        invalid_mode,
                    )
                }
            },
            None => "on-demand".to_string(), // Default when not specified
        };

        // Validade node_affinity
        if cluster_yaml.use_node_affinity && !instance_type_details.has_affinity_settings {
            bail!(
                "Instance type '{}' does not support node affinity settings",
                &instance_type_name
            )
        }

        // Validate elastic fabric adapters support
        if cluster_yaml.use_elastic_fabric_adapters && !instance_type_details.supports_efa {
            bail!(
                "Instance type '{}' does not support elastic fabric adapters",
                &instance_type_name
            )
        }

        // Validate burstable_mode
        let burstable_mode = match &node_definition.burstable_mode {
            Some(burstable_mode) => match instance_type_details.is_burstable {
                true => {
                    let valid_modes = ["unlimited", "standard"];
                    if !valid_modes.contains(&burstable_mode.to_lowercase().as_str()) {
                        bail!(
                            "Invalid burstable mode '{}' specified for node '{}'.\
                            The instance type '{}' in region '{}' only supports the following burstale modes: {}",
                            burstable_mode,
                            i + 1,
                            &instance_type_name,
                            &region,
                            valid_modes.join(", ")
                        )
                    }
                    Some(burstable_mode)
                }
                false => {
                    bail!(
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
        nodes_tracker.inc(1);
    }
    nodes_tracker.finish_with_message(&format!("Validated {} nodes", node_count));

    // TODO: find a way to remove the code duplication here and in `database/models/cluster.rs`
    println!("\n{:<35}: {}", "Cluster Name", cluster_yaml.display_name);
    println!("{:<35}: {}", "Provider", provider_config.provider_id);
    println!("{:<35}: {}", "Region", region);
    println!(
        "{:<35}: {}",
        "Availability Zone", cluster_yaml.availability_zone
    );
    println!(
        "{:<35}: {}",
        "Use Node Affinity", cluster_yaml.use_node_affinity
    );
    println!(
        "{:<35}: {}",
        "Use Elastic Fabric Adapters (EFAs)", cluster_yaml.use_elastic_fabric_adapters
    );
    println!(
        "{:<35}: {}",
        "Use Elastic File System (EFS)", cluster_yaml.use_elastic_file_system
    );
    println!(
        "{:<35}: {}",
        "On Instance Creation Failure", cluster_yaml.on_instance_creation_failure
    );
    println!(
        "{:<35}: {}",
        "Provider Config", provider_config.display_name
    );
    println!("{:<35}: {}\n", "Node Count", cluster_yaml.nodes.len());

    println!("Node Details:");
    for (i, node) in cluster_yaml.nodes.iter().enumerate() {
        let instance_type_name = &node.instance_type;
        let instance_details =
            InstanceType::fetch_by_name_and_region(pool, instance_type_name, &region)
                .await?
                .unwrap(); // Because of the previous validation, unwrap won't fail here

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

        println!("  Node {}:", i + 1);
        println!("    Instance Type   : {}", node.instance_type);
        println!("    Processor       : {}", processor_info);
        println!("    vCPUs:          : {}", instance_details.vcpus);
        println!("    GPUs:           : {}", gpu_info);
        println!("    Image ID        : {}", node.image_id);
        println!(
            "    Allocation Mode : {}",
            node.allocation_mode.as_deref().unwrap_or("on-demand")
        );
        println!(
            "    Burstable Mode  : {}",
            node.burstable_mode.as_deref().unwrap_or("N/A")
        );
        println!();
    }

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed creating this cluster?",
    )?) {
        return Ok(());
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
        availability_zone: zone,
        use_node_affinity: cluster_yaml.use_node_affinity,
        use_elastic_fabric_adapters: cluster_yaml.use_elastic_fabric_adapters,
        use_elastic_file_system: cluster_yaml.use_elastic_file_system,
        created_at: Utc::now().naive_utc(),
        state: ClusterState::Pending,
        on_instance_creation_failure: Some(failure_policy.clone()), // 'cancel' as default
        migration_attempts: 0,
        tried_zones: Some("".to_string()),
    };
    cluster
        .insert(pool, nodes_to_insert, commands_to_insert)
        .await?;

    println!(
        "New Cluster '{}' created successfully! To spawn this cluster, \
        use: 'cluster spawn --cluster-id {}'",
        cluster_name, cluster.id
    );
    Ok(())
}
