use crate::database::models::{Cluster, ClusterState, ProviderConfig};
use crate::integrations::providers::aws::AwsInterface;
use crate::utils;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

/// Injects a spot interruption notice for one node into the cluster's own
/// interruption queue, without terminating anything.
///
/// This is the notice half of V-B scenario (i): `cluster watch` reacts exactly as
/// it would to a real AWS warning, flushing a preemptive checkpoint and starting
/// to provision the replacement. Follow it with `cluster test-failure` on the same
/// node after the provider's warning window to reproduce the reclamation itself.
pub async fn simulate_spot_notice(
    pool: &SqlitePool,
    cluster_id: &str,
    node_private_ip: &str,
    skip_confirmation: bool,
) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            tracing::warn!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    match cluster.state {
        ClusterState::Running | ClusterState::Restoring => {
            tracing::info!(
                "Injecting spot interruption notice into Cluster '{}'...",
                cluster.display_name
            )
        }
        _ => {
            anyhow::bail!(
                "Cluster '{}' is '{}'. A notice can only be injected while it is running.",
                cluster.display_name,
                cluster.state
            );
        }
    }

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
        "Do you confirm you want to inject a spot interruption notice for this node?",
    )?) {
        return Ok(());
    }

    cloud_interface
        .send_simulated_spot_interruption(&cluster, node_private_ip)
        .await?;

    tracing::info!(
        "Notice delivered. 'cluster watch' should now signal the MPI job to checkpoint \
         and begin provisioning a replacement."
    );
    Ok(())
}
