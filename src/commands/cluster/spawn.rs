use crate::database::models::{Cluster, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;

use sqlx::sqlite::SqlitePool;
use tracing::error;

pub async fn spawn(
    pool: &SqlitePool,
    cluster_id: &str,
    skip_confirmation: bool,
) -> anyhow::Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await {
        Ok(Some(cluster)) => cluster,
        Ok(None) => {
            anyhow::bail!("Cluster '{}' not found.", cluster_id)
        }
        Err(e) => {
            error!("SQLx Error: {}", e.to_string());
            anyhow::bail!("DB Operation Failure");
        }
    };

    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => {
                error!("Missing ProviderConfig '{}'", cluster.provider_config_id);
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

    let nodes = cluster.get_nodes(pool).await?;
    cluster.print_details(pool).await?;

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed spawning this cluster?",
    )?) {
        return Ok(());
    }

    cloud_interface.spawn_cluster(cluster, nodes).await?;
    Ok(())
}
