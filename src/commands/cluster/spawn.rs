use crate::database::models::{Cluster, ClusterState, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn spawn(pool: &SqlitePool, cluster_id: &str, skip_confirmation: bool) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            bail!("Cluster (id='{}') not found", cluster_id);
        }
    };

    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => {
                bail!(
                    "ProviderConfig (id='{}') not found",
                    cluster.provider_config_id
                );
            }
        };

    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars, db_pool: Arc::new(pool.clone()) },
        _ => {
            bail!(
                "Provider (id='{}') is currently not supported.",
                &provider_id
            )
        }
    };

    let nodes = cluster.get_nodes(pool).await?;
    cluster.print_details(pool).await?;

    let mut init_commands_map: HashMap<usize, Vec<String>> = HashMap::new();
    for (node_index, node) in nodes.iter().enumerate() {
        let node_commands = node.get_init_commands(pool).await?;
        init_commands_map.insert(node_index, node_commands);
    }

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed spawning this cluster?",
    )?) {
        return Ok(());
    }

    Cluster::update_cluster_state(pool, cluster_id, ClusterState::Spawning).await?;
    cloud_interface
        .spawn_cluster(cluster, nodes, init_commands_map)
        .await?;
    Cluster::update_cluster_state(pool, cluster_id, ClusterState::Running).await?;
    Ok(())
}
