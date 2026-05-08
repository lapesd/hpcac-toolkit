use crate::database::models::{Cluster, ClusterState, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

pub async fn terminate(pool: &SqlitePool, cluster_id: &str, skip_confirmation: bool) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            tracing::warn!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    match cluster.state {
        ClusterState::Terminated => {
            tracing::info!("Cluster '{}' is already terminated.", cluster.display_name);
            return Ok(());
        }
        _ => {
            tracing::info!("Terminating Cluster '{}'...", cluster.display_name)
        }
    }

    let nodes = cluster.get_nodes(pool).await?;
    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => {
                tracing::error!("Missing ProviderConfig '{}'", cluster.provider_config_id);
                anyhow::bail!("Data Consistency Failure");
            }
        };

    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => {
            anyhow::bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you confirm you want to terminate this cluster?",
    )?) {
        return Ok(());
    }

    cloud_interface
        .terminate_cluster(pool, cluster, nodes)
        .await?;
    Ok(())
}
