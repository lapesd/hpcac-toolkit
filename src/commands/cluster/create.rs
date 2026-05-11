use crate::database::models::{
    Cluster, ClusterState, InstanceType, Node, Provider, ProviderConfig, ShellCommand,
};
use crate::integrations::{cloud_interface::CloudInfoProvider, providers::aws::AwsInterface};
use crate::utils;

use anyhow::Result;
use chrono::Utc;
use inquire::Select;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::fs;
use std::path::Path;
use serde_json;

const GP3_THROUGHPUT_BASELINE_MBS: f64 = 125.0;
const GP3_PROVISIONED_THROUGHPUT_MBS: f64 = 500.0;
const GP3_IOPS_BASELINE: f64 = 3000.0;
const GP3_IOPS_PER_MONTH: f64 = 0.005;
const IO1_IO2_IOPS_PER_MONTH: f64 = 0.065;
const PUBLIC_IPV4_PER_HOUR: f64 = 0.005;
const HOURS_PER_MONTH: f64 = 730.0;

#[derive(Serialize, Deserialize)]
struct NodeCostBreakdown {
    node_id: String,
    instance_type: String,
    allocation_mode: String,
    instance_cost_per_hour: f64,
    ebs_cost_per_hour: f64,
    iops_cost_per_hour: f64,
    public_ip_cost_per_hour: f64,
    node_total_per_hour: f64,
}

#[derive(Serialize, Deserialize)]
struct CostBreakdown {
    nodes: Vec<NodeCostBreakdown>,
    total_per_hour: f64,
}

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
    efs_performance_mode: Option<String>,
    efs_throughput_mode: Option<String>,
    efs_provisioned_throughput_mbs: Option<f64>,
    nodes: Vec<NodeYaml>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeYaml {
    instance_type: String,
    allocation_mode: Option<String>,
    burstable_mode: Option<String>,
    image_id: String,
    root_volume_gb: Option<i64>,
    root_volume_type: Option<String>,
    root_volume_iops: Option<i64>,
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
            tracing::info!("Successfully read file: '{}'", yaml_file_path);
            result
        }
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!("Failed to read file '{}'", yaml_file_path)
        }
    };

    let cluster_yaml: ClusterYaml = match serde_yaml::from_str(&cluster_yaml_str) {
        Ok(result) => {
            tracing::info!("Parsed cluster yaml file successfully");
            result
        }
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!(
                "Failed to parse yaml file: '{}': {:?}",
                yaml_file_path,
                e.to_string()
            )
        }
    };

    // Validate cluster.id
    let new_cluster_id = match cluster_yaml.id {
        Some(id) => {
            if id.is_empty() {
                anyhow::bail!("Cluster ID cannot be empty");
            }

            for (i, ch) in id.chars().enumerate() {
                if !ch.is_alphanumeric() && ch != '-' && ch != '_' {
                    anyhow::bail!(
                        "Invalid character '{}' at position {} in cluster ID '{}'. Only \
                        alphanumeric characters, hyphens (-), and underscores (_) are allowed",
                        ch,
                        i,
                        id
                    )
                }
            }

            let existing_cluster = Cluster::fetch_by_id(pool, &id).await?;
            if existing_cluster.is_some() {
                anyhow::bail!(
                    "Cluster with id: '{}' already exists. Please update the yaml file and try \
                    again",
                    id
                );
            }
            id
        }
        None => utils::generate_id(),
    };

    // Validate cluster.display_name
    let cluster_name = cluster_yaml.display_name.clone();
    let existing_cluster = Cluster::fetch_by_name(pool, &cluster_name).await?;
    if existing_cluster.is_some() {
        anyhow::bail!(
            "Cluster with display_name: '{}' already exists. Please update the yaml file and try \
            again.",
            cluster_name
        );
    }

    // Validate provided SSH key pair
    let public_key_path_string = utils::expand_tilde(&cluster_yaml.public_ssh_key_path);
    let public_key_path = Path::new(&public_key_path_string);
    let _public_ssh_key = match fs::read_to_string(public_key_path) {
        Ok(result) => {
            tracing::info!("Successfully read file: '{}'", &public_key_path_string);
            result
        }
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!("Failed to read file: '{}'", &public_key_path_string)
        }
    };
    let private_key_path_string = utils::expand_tilde(&cluster_yaml.private_ssh_key_path);
    let private_key_path = Path::new(&private_key_path_string);
    let _private_ssh_key = match fs::read_to_string(private_key_path) {
        Ok(result) => {
            tracing::info!("Successfully read file: '{}'", &private_key_path_string);
            result
        }
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!("Failed to read file: '{}'", &private_key_path_string)
        }
    };

    // Validate provider_config if provided, else prompt user for selection
    let provider_config = match cluster_yaml.provider_config_id {
        Some(config_id) => {
            let config_query = ProviderConfig::fetch_by_id(pool, config_id).await?;
            match config_query {
                Some(result) => {
                    tracing::info!("Provider Configuration: '{}' found", config_id);
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
                            tracing::info!("Provider: '{}' found", provider_id);
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
                            tracing::error!("{}", e.to_string());
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

    tracing::info!("Validating cloud provider connection and cluster node data...");

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
    let zone = cluster_yaml.availability_zone.clone();
    if zone.is_empty() {
        anyhow::bail!("availability_zone is required");
    }
    let zones_tracker = utils::ProgressTracker::new(1, Some("zones discovery"));
    let zones = cloud_interface.fetch_zones(&region, &zones_tracker).await?;
    zones_tracker.finish_with_message(&format!(
        "Zone discovery complete: found {} zones in {}",
        zones.len(),
        region
    ));
    if !zones.contains(&zone) {
        anyhow::bail!(
            "Availability Zone '{}' is not available. Possible options: {:?}",
            zone,
            zones
        )
    }

    // Validate EFS configuration
    let efs_performance_mode = match &cluster_yaml.efs_performance_mode {
        Some(mode) => match mode.to_lowercase().as_str() {
            "general_purpose" | "generalpurpose" => "general_purpose".to_string(),
            "max_io" | "maxio" => "max_io".to_string(),
            invalid => anyhow::bail!(
                "Invalid efs_performance_mode '{}'. Valid options: general_purpose, max_io",
                invalid
            ),
        },
        None => "general_purpose".to_string(),
    };
    let efs_throughput_mode = match &cluster_yaml.efs_throughput_mode {
        Some(mode) => match mode.to_lowercase().as_str() {
            "bursting" => "bursting".to_string(),
            "provisioned" => "provisioned".to_string(),
            "elastic" => "elastic".to_string(),
            invalid => anyhow::bail!(
                "Invalid efs_throughput_mode '{}'. Valid options: bursting, provisioned, elastic",
                invalid
            ),
        },
        None => "bursting".to_string(),
    };
    let efs_provisioned_throughput_mbs = cluster_yaml.efs_provisioned_throughput_mbs;
    if efs_throughput_mode == "provisioned" && efs_provisioned_throughput_mbs.is_none() {
        anyhow::bail!("efs_provisioned_throughput_mbs is required when efs_throughput_mode is 'provisioned'");
    }
    if efs_throughput_mode != "provisioned" && efs_provisioned_throughput_mbs.is_some() {
        anyhow::bail!("efs_provisioned_throughput_mbs can only be set when efs_throughput_mode is 'provisioned'");
    }

    // Validate node data
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
                tracing::error!("{}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e)
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

        // Validate node_affinity
        if cluster_yaml.use_node_affinity && !instance_type_details.has_affinity_settings {
            anyhow::bail!(
                "Instance type '{}' does not support node affinity settings",
                &instance_type_name
            )
        }

        // Validate elastic fabric adapters support
        if cluster_yaml.use_elastic_fabric_adapters && !instance_type_details.supports_efa {
            anyhow::bail!(
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
                        anyhow::bail!(
                            "Invalid burstable mode '{}' specified for node '{}'.\
                            The instance type '{}' in region '{}' only supports the following \
                            burstable modes: {}",
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

        // Validate root_volume_type
        let root_volume_type = match &node_definition.root_volume_type {
            Some(vt) => {
                let valid_types = ["gp2", "gp3", "io1", "io2"];
                if !valid_types.contains(&vt.to_lowercase().as_str()) {
                    anyhow::bail!(
                        "Invalid root_volume_type '{}' for node {}. Valid options: {}",
                        vt,
                        i + 1,
                        valid_types.join(", ")
                    )
                }
                vt.to_lowercase()
            }
            None => "gp3".to_string(),
        };

        // Validate root_volume_gb
        let root_volume_gb = match node_definition.root_volume_gb {
            Some(gb) if gb < 8 => {
                anyhow::bail!(
                    "root_volume_gb for node {} must be at least 8 GB, got {}",
                    i + 1,
                    gb
                )
            }
            Some(gb) => gb,
            None => 100,
        };

        // Validate root_volume_iops
        let root_volume_iops = match (node_definition.root_volume_iops, root_volume_type.as_str()) {
            (Some(iops), "gp3") if iops > 16000 => {
                anyhow::bail!("root_volume_iops for node {} exceeds gp3 maximum of 16000", i + 1)
            }
            (Some(iops), "io1") if iops > 64000 => {
                anyhow::bail!("root_volume_iops for node {} exceeds io1 maximum of 64000", i + 1)
            }
            (Some(iops), "io2") if iops > 256000 => {
                anyhow::bail!("root_volume_iops for node {} exceeds io2 maximum of 256000", i + 1)
            }
            (Some(_), "gp2") => {
                anyhow::bail!("root_volume_iops cannot be set for gp2 volumes (node {})", i + 1)
            }
            (Some(iops), _) if iops < 100 => {
                anyhow::bail!("root_volume_iops for node {} must be at least 100", i + 1)
            }
            (iops, _) => iops,
        };

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
            instance_type: instance_type_name,
            allocation_mode,
            burstable_mode: burstable_mode.cloned(),
            image_id,
            root_volume_gb,
            root_volume_type,
            root_volume_iops,
            private_ip: None,
            public_ip: None,
            was_efs_configured: false,
            was_ssh_configured: false,
        });
        nodes_tracker.inc(1);
    }
    nodes_tracker.finish_with_message(&format!("Validated {} nodes", node_count));

    // Compute cost breakdown — fetch live prices from AWS Pricing API
    let unique_instance_types: Vec<String> = nodes_to_insert
        .iter()
        .map(|n| n.instance_type.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let instance_pricing_tracker = utils::ProgressTracker::new(
        unique_instance_types.len() as u64,
        Some("instance pricing"),
    );
    let live_instance_prices = cloud_interface
        .fetch_prices(&region, &unique_instance_types, &instance_pricing_tracker)
        .await?;
    instance_pricing_tracker.finish_with_message("Instance pricing fetched");

    let spot_instance_types: Vec<String> = nodes_to_insert
        .iter()
        .filter(|n| n.allocation_mode == "spot")
        .map(|n| n.instance_type.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let live_spot_prices = if !spot_instance_types.is_empty() {
        cloud_interface.fetch_spot_prices(&region, &spot_instance_types, &zone).await?
    } else {
        std::collections::HashMap::new()
    };

    let unique_volume_types: std::collections::HashSet<String> =
        nodes_to_insert.iter().map(|n| n.root_volume_type.clone()).collect();
    let mut ebs_price_cache: std::collections::HashMap<String, crate::integrations::providers::aws::EbsPricing> =
        std::collections::HashMap::new();
    for vt in &unique_volume_types {
        ebs_price_cache.insert(vt.clone(), cloud_interface.fetch_ebs_pricing(&region, vt).await?);
    }

    let mut cost_nodes: Vec<NodeCostBreakdown> = vec![];
    for node in &nodes_to_insert {
        let instance_cost_per_hour = if node.allocation_mode == "spot" {
            *live_spot_prices.get(&node.instance_type).unwrap_or(
                live_instance_prices.get(&node.instance_type).unwrap_or(&0.0),
            )
        } else {
            *live_instance_prices.get(&node.instance_type).unwrap_or(&0.0)
        };

        let ebs_pricing = ebs_price_cache.get(&node.root_volume_type).unwrap();
        let storage_cost = node.root_volume_gb as f64 * ebs_pricing.storage_per_gb_month / HOURS_PER_MONTH;
        let extra_throughput = (GP3_PROVISIONED_THROUGHPUT_MBS - GP3_THROUGHPUT_BASELINE_MBS).max(0.0);
        let throughput_cost = extra_throughput * ebs_pricing.throughput_per_mbs_month / HOURS_PER_MONTH;
        let ebs_cost_per_hour = storage_cost + throughput_cost;

        let iops_cost_per_hour = match (node.root_volume_iops, node.root_volume_type.as_str()) {
            (Some(iops), "gp3") => {
                let extra = (iops as f64 - GP3_IOPS_BASELINE).max(0.0);
                extra * GP3_IOPS_PER_MONTH / HOURS_PER_MONTH
            }
            (Some(iops), "io1") | (Some(iops), "io2") => {
                iops as f64 * IO1_IO2_IOPS_PER_MONTH / HOURS_PER_MONTH
            }
            _ => 0.0,
        };

        let node_total_per_hour = instance_cost_per_hour + ebs_cost_per_hour + iops_cost_per_hour + PUBLIC_IPV4_PER_HOUR;

        cost_nodes.push(NodeCostBreakdown {
            node_id: node.id.clone(),
            instance_type: node.instance_type.clone(),
            allocation_mode: node.allocation_mode.clone(),
            instance_cost_per_hour,
            ebs_cost_per_hour,
            iops_cost_per_hour,
            public_ip_cost_per_hour: PUBLIC_IPV4_PER_HOUR,
            node_total_per_hour,
        });
    }
    let total_per_hour = cost_nodes.iter().map(|n| n.node_total_per_hour).sum();
    let cost_breakdown = serde_json::to_string(&CostBreakdown {
        nodes: cost_nodes,
        total_per_hour,
    })?;

    // TODO: find a way to remove the code duplication here and in `database/models/cluster.rs`
    tracing::info!(
        "\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n\nNode Details:",
        "Cluster Name",
        cluster_yaml.display_name,
        "Provider",
        provider_config.provider_id,
        "Region",
        region,
        "Availability Zone",
        cluster_yaml.availability_zone,
        "Use Node Affinity",
        cluster_yaml.use_node_affinity,
        "Use Elastic Fabric Adapters (EFAs)",
        cluster_yaml.use_elastic_fabric_adapters,
        "Use Elastic File System (EFS)",
        cluster_yaml.use_elastic_file_system,
        "Provider Config",
        provider_config.display_name,
        "Node Count",
        cluster_yaml.nodes.len()
    );
    for (i, node) in cluster_yaml.nodes.iter().enumerate() {
        let instance_type_name = &node.instance_type;
        let instance_details =
            InstanceType::fetch_by_name_and_region(pool, instance_type_name, &region)
                .await?
                .unwrap(); // Because of the previous validation, unwrap won't fail here

        let processor_info = utils::format_processor_info(
            instance_details.core_count,
            instance_details.cpu_architecture,
            instance_details.cpu_type,
        );

        let gpu_info = match instance_details.gpu_type {
            Some(gpu) => {
                format!("{}x {}", instance_details.gpu_count, gpu)
            }
            None => "N/A".to_string(),
        };

        tracing::info!(
            "  Node {}:\n    Instance Type   : {}\n    Processor       : {}\n    vCPUs:          : {}\n    GPUs:           : {}\n    Image ID        : {}\n    Allocation Mode : {}\n    Burstable Mode  : {}",
            i + 1,
            node.instance_type,
            processor_info,
            instance_details.vcpus,
            gpu_info,
            node.image_id,
            node.allocation_mode.as_deref().unwrap_or("on-demand"),
            node.burstable_mode.as_deref().unwrap_or("N/A")
        );
    }

    if !utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed creating this cluster?",
    )? {
        return Ok(());
    }

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
        efs_performance_mode,
        efs_throughput_mode,
        efs_provisioned_throughput_mbs,
        created_at: Utc::now().naive_utc(),
        state: ClusterState::Pending,
        cost_per_hour: total_per_hour,
        cost_breakdown,
    };
    cluster
        .insert(pool, nodes_to_insert, commands_to_insert)
        .await?;

    tracing::info!(
        "New Cluster '{}' created successfully! To spawn this cluster, \
        use: 'cluster spawn --cluster-id {}'",
        cluster_name,
        cluster.id
    );
    Ok(())
}
