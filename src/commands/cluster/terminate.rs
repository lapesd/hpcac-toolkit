use crate::database::models::{Cluster, ClusterState, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use tracing::{error, info};
use std::sync::Arc;

pub async fn terminate(pool: &SqlitePool, cluster_id: &str, skip_confirmation: bool) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            println!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    match cluster.state {
        ClusterState::Terminated => {
            println!("Cluster '{}' is already terminated.", cluster.display_name);
            return Ok(());
        }
        _ => {
            info!("Terminating Cluster '{}'...", cluster.display_name)
        }
    }

    let nodes = cluster.get_nodes(pool).await?;
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
        "aws" => AwsInterface { config_vars, db_pool: Arc::new(pool.clone()) },
        _ => {
            bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you confirm you want to terminate this cluster?",
    )?) {
        return Ok(());
    }

    Cluster::update_cluster_state(pool, cluster_id, ClusterState::Terminating).await?;
    cloud_interface.destroy_cluster(cluster, nodes).await?;
    Cluster::update_cluster_state(pool, cluster_id, ClusterState::Terminated).await?;
    Ok(())
}
