use crate::database::models::{Cluster, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils;
use sqlx::sqlite::SqlitePool;
use tracing::error;

pub async fn spawn(
    pool: &SqlitePool,
    blueprint_id: &str,
    skip_confirmation: bool,
) -> anyhow::Result<()> {
    let cluster = Cluster::fetch_by_id(pool, blueprint_id).await?;
    let provider_config = match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await
    {
        Ok(Some(result)) => result,
        Ok(None) => {
            anyhow::bail!(
                "Missing Provider Configuration: '{}'",
                cluster.provider_config_id
            )
        }
        Err(e) => {
            error!("{}", e.to_string());
            anyhow::bail!("DB Operation Failure")
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

    /*
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
    */

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
