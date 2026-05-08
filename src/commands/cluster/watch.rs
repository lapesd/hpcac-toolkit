use crate::database::models::{Cluster, ClusterState, Node, ProviderConfig};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};

use anyhow::Result;
use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use tokio::time::{Duration, sleep};

pub async fn watch(pool: &SqlitePool, cluster_id: &str, interval_secs: u64) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(c) => c,
        None => anyhow::bail!("Cluster (id='{}') not found", cluster_id),
    };

    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => anyhow::bail!("ProviderConfig not found for Cluster (id='{}')", cluster_id),
        };

    let config_vars = provider_config.get_config_vars(pool).await?;
    let cloud_interface = match provider_config.provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => anyhow::bail!(
            "Provider '{}' is currently not supported.",
            &provider_config.provider_id
        ),
    };

    tracing::info!(
        "Monitoring Cluster '{}' every {}s. Press Ctrl+C to stop.",
        cluster_id,
        interval_secs
    );

    loop {
        let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
            Some(c) => c,
            None => anyhow::bail!("Cluster (id='{}') disappeared from database", cluster_id),
        };

        match cluster.state {
            ClusterState::Terminated | ClusterState::Terminating => {
                tracing::info!(
                    "[{}] Cluster '{}' is {}, stopping monitor.",
                    Utc::now().format("%H:%M:%S"),
                    cluster_id,
                    cluster.state
                );
                break;
            }
            ClusterState::Spawning | ClusterState::Restoring => {
                tracing::info!(
                    "[{}] Cluster '{}' is {} — waiting for provisioning to complete...",
                    Utc::now().format("%H:%M:%S"),
                    cluster_id,
                    cluster.state
                );
                sleep(Duration::from_secs(interval_secs)).await;
                continue;
            }
            ClusterState::Running => {}
            ref other => {
                tracing::info!(
                    "[{}] Cluster '{}' is in state '{}', skipping health check.",
                    Utc::now().format("%H:%M:%S"),
                    cluster_id,
                    other
                );
                sleep(Duration::from_secs(interval_secs)).await;
                continue;
            }
        }

        match cloud_interface.check_cluster_health(pool, &cluster).await {
            Ok(failed_ips) if failed_ips.is_empty() => {
                tracing::info!(
                    "[{}] Cluster '{}' — all nodes healthy.",
                    Utc::now().format("%H:%M:%S"),
                    cluster_id
                );
            }
            Ok(failed_ips) => {
                tracing::warn!(
                    "[{}] Detected {} failed node(s): {:?} — triggering restore...",
                    Utc::now().format("%H:%M:%S"),
                    failed_ips.len(),
                    failed_ips
                );

                for ip in &failed_ips {
                    match Node::fetch_by_private_ip(pool, ip).await? {
                        Some(node) => {
                            node.set_efs_configuration_state(pool, false).await?;
                            tracing::info!(
                                "Marked node (private_ip='{}') for EFS re-configuration",
                                ip
                            );
                        }
                        None => {
                            tracing::warn!(
                                "No node record found for private_ip='{}', skipping.",
                                ip
                            );
                        }
                    }
                }

                let nodes = cluster.get_nodes(pool).await?;
                match cloud_interface.spawn_cluster(pool, cluster, nodes).await {
                    Ok(()) => {
                        tracing::info!(
                            "[{}] Restore completed successfully.",
                            Utc::now().format("%H:%M:%S")
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "[{}] Restore failed: {}",
                            Utc::now().format("%H:%M:%S"),
                            e
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "[{}] Health check error (will retry): {}",
                    Utc::now().format("%H:%M:%S"),
                    e
                );
            }
        }

        sleep(Duration::from_secs(interval_secs)).await;
    }

    Ok(())
}
