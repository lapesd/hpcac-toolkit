use crate::database::models::{Cluster, ClusterState, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use tracing::{error, info};

pub async fn test_failure(
    pool: &SqlitePool,
    cluster_id: &str,
    node_private_ip: &str,
    skip_confirmation: bool,
) -> Result<()> {
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
            info!(
                "Simulating Spot failure in Cluster '{}'...",
                cluster.display_name
            )
        }
    }

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

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you confirm you want to simulate a failure in this cluster?",
    )?) {
        return Ok(());
    }

    cloud_interface
        .simulate_cluster_failure(cluster, node_private_ip)
        .await?;
    Ok(())
}
