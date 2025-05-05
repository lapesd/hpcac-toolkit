use crate::database::models::{Cluster, ProviderConfig};
use crate::integrations::{CloudInterface, providers::aws::AwsInterface};
use inquire::Confirm;
use sqlx::sqlite::SqlitePool;
use tracing::{error, info};

pub async fn spawn(
    pool: &SqlitePool,
    cluster_id: &str,
    skip_confirmation: bool,
) -> anyhow::Result<()> {
    let cluster = Cluster::fetch_by_id(pool, cluster_id).await?;
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

    // Get cloud interface
    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => {
            anyhow::bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    let nodes = cluster.get_nodes(pool).await?;
    cloud_interface.spawn_cluster(cluster, nodes).await?;

    if !skip_confirmation {
        let confirm = Confirm::new("Do you want to proceed with spawning this cluster?")
            .with_default(true)
            .prompt();
        match confirm {
            Ok(true) => info!("Confirmed! Proceeding with cluster spawn..."),
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

    println!("Spawning cluster {}", cluster_id);

    Ok(())
}
