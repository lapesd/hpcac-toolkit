use crate::commands::cluster::tasks::tasks;
use crate::database::models::{Cluster, ClusterState, Node, ProviderConfig, RecoveryNode};
use crate::integrations::{cloud_interface::CloudResourceManager, providers::aws::AwsInterface};
use crate::utils::{self, ssh::SshSession};

use anyhow::Result;
use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

pub async fn watch(
    pool: &SqlitePool,
    cluster_id: &str,
    interval_secs: u64,
    tasks_yaml: Option<&str>,
    no_replace: bool,
) -> Result<()> {
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

    // Discover the spot interruption queue created at spawn time (None if no spot nodes)
    let queue_url = match cloud_interface
        .get_spot_interruption_queue_url(cluster_id, &cluster.region)
        .await
    {
        Ok(Some(url)) => {
            tracing::info!("Spot interruption alerts active.");
            Some(url)
        }
        Ok(None) => {
            tracing::info!("No spot interruption queue found — cluster has no spot nodes.");
            None
        }
        Err(e) => {
            tracing::warn!("Could not resolve spot interruption queue (alerts disabled): {}", e);
            None
        }
    };

    if tasks_yaml.is_some() {
        tracing::info!("Automatic relaunch enabled after restore.");
    }
    if no_replace {
        tracing::info!("Scale-down mode: failed nodes will be removed and job relaunched on remaining nodes.");
    }

    tracing::info!(
        "Monitoring Cluster '{}' every {}s. Press Ctrl+C to stop.",
        cluster_id,
        interval_secs
    );

    let private_key_path = utils::expand_tilde(&cluster.private_ssh_key_path);
    let pool = Arc::new(pool.clone());
    let tasks_yaml: Option<Arc<str>> = tasks_yaml.map(|s| Arc::from(s));

    // Tracks any in-flight tasks run so we can abort it before triggering a new restore
    let mut tasks_handle: Option<JoinHandle<()>> = None;

    loop {
        let cluster = match Cluster::fetch_by_id(&pool, cluster_id).await? {
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

        // Check for incoming spot interruption notices before the reactive health check
        if let Some(url) = &queue_url {
            match cloud_interface
                .poll_spot_interruption_queue(cluster_id, &cluster.region, url)
                .await
            {
                Ok(interrupted_ips) if !interrupted_ips.is_empty() => {
                    tracing::warn!(
                        "[{}] Spot interruption notice(s) for {} node(s): {:?} — signalling MPI job to checkpoint...",
                        Utc::now().format("%H:%M:%S"),
                        interrupted_ips.len(),
                        interrupted_ips
                    );
                    signal_mpi_checkpoint(&pool, &cluster, &private_key_path).await;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!(
                        "[{}] Failed to poll spot interruption queue: {}",
                        Utc::now().format("%H:%M:%S"),
                        e
                    );
                }
            }
        }

        match cloud_interface.check_cluster_health(&pool, &cluster).await {
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

                // Abort any in-flight tasks run — the MPI job is already dead
                if let Some(handle) = tasks_handle.take() {
                    handle.abort();
                    tracing::info!("Aborted previous tasks run.");
                }

                for ip in &failed_ips {
                    match Node::fetch_by_private_ip(&pool, ip).await? {
                        Some(node) => {
                            node.set_efs_configuration_state(&pool, false).await?;
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

                // Wait for terminated instances to fully release their ENIs before
                // attempting to reuse them for the replacement node.
                tracing::info!(
                    "[{}] Waiting 30s for failed instance(s) to release network interfaces...",
                    Utc::now().format("%H:%M:%S")
                );
                sleep(Duration::from_secs(30)).await;

                if no_replace {
                    // Scale-down: remove failed nodes from DB and relaunch on survivors.
                    // Delete the orphaned ENI first — the node's DB record is removed by
                    // delete_by_private_ip, but the standalone ENI persists in AWS and
                    // blocks security-group/subnet deletion on cluster termination.
                    for ip in &failed_ips {
                        if let Err(e) = cloud_interface
                            .delete_detached_eni_by_private_ip(&cluster.region, ip)
                            .await
                        {
                            tracing::warn!("Could not delete ENI for node '{}': {}", ip, e);
                        }
                        if let Err(e) = Node::delete_by_private_ip(&pool, ip).await {
                            tracing::warn!("Could not remove node '{}' from DB: {}", ip, e);
                        } else {
                            tracing::info!("Removed failed node '{}' from cluster.", ip);
                        }
                    }
                    cluster.update_state(&pool, ClusterState::Running).await?;
                    tracing::info!(
                        "[{}] Scale-down complete — {} node(s) remaining. Relaunching job...",
                        Utc::now().format("%H:%M:%S"),
                        cluster.get_nodes(&pool).await?.len()
                    );
                    if let Some(yaml_path) = tasks_yaml.clone() {
                        let pool_clone = Arc::clone(&pool);
                        tasks_handle = Some(tokio::spawn(async move {
                            tracing::info!("Relaunching MPI job from '{}'...", yaml_path);
                            if let Err(e) = tasks(&pool_clone, &yaml_path).await {
                                tracing::warn!("Tasks relaunch ended: {}", e);
                            }
                        }));
                    }
                } else {
                    let nodes = apply_recovery_policy(
                        &pool,
                        &cluster,
                        cluster.get_nodes(&pool).await?,
                        &failed_ips,
                    )
                    .await?;

                    match cloud_interface.spawn_cluster(&pool, cluster, nodes).await {
                        Ok(()) => {
                            tracing::info!(
                                "[{}] Restore completed successfully.",
                                Utc::now().format("%H:%M:%S")
                            );
                            if let Some(yaml_path) = tasks_yaml.clone() {
                                let pool_clone = Arc::clone(&pool);
                                tasks_handle = Some(tokio::spawn(async move {
                                    tracing::info!("Relaunching MPI job from '{}'...", yaml_path);
                                    if let Err(e) = tasks(&pool_clone, &yaml_path).await {
                                        tracing::warn!("Tasks relaunch ended: {}", e);
                                    }
                                }));
                            }
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

/// Applies the cluster's recovery policy to failed node slots.
///
/// Nodes are sorted by private IP (consistent with tasks.rs) to establish stable
/// slot indices. For each failed node, the corresponding recovery node slot (by
/// index) is used to update the node's instance type and allocation mode in the DB.
/// Returns the refreshed node list to pass to spawn_cluster.
async fn apply_recovery_policy(
    pool: &SqlitePool,
    cluster: &Cluster,
    mut nodes: Vec<Node>,
    failed_ips: &[String],
) -> anyhow::Result<Vec<Node>> {
    let recovery_nodes = RecoveryNode::fetch_all_by_cluster_id(pool, &cluster.id).await?;
    if recovery_nodes.is_empty() {
        return Ok(nodes);
    }

    nodes.sort_by(|a, b| a.private_ip.cmp(&b.private_ip));

    for (slot_index, node) in nodes.iter().enumerate() {
        let ip = match node.private_ip.as_deref() {
            Some(ip) => ip,
            None => continue,
        };
        if !failed_ips.contains(&ip.to_string()) {
            continue;
        }
        let Some(recovery) = recovery_nodes.get(slot_index) else {
            tracing::warn!(
                "No recovery node defined for slot {}, keeping original spec for '{}'",
                slot_index,
                ip
            );
            continue;
        };
        let Some(instance_type) = recovery.primary_instance_type() else {
            tracing::warn!(
                "Recovery slot {} has no preferred_instance_types, keeping original spec",
                slot_index
            );
            continue;
        };
        tracing::info!(
            "Applying recovery policy to slot {} (ip='{}'): {} / {}",
            slot_index,
            ip,
            instance_type,
            recovery.allocation_mode
        );
        node.update_instance_spec(pool, &instance_type, &recovery.allocation_mode)
            .await?;
    }

    Ok(cluster.get_nodes(pool).await?)
}

/// SSH to the head node and send SIGUSR1 to the running mpirun process so the
/// application can flush a checkpoint before the spot instance is reclaimed.
async fn signal_mpi_checkpoint(pool: &SqlitePool, cluster: &Cluster, private_key_path: &str) {
    let mut nodes = match cluster.get_nodes(pool).await {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("Could not fetch nodes for checkpoint signal: {}", e);
            return;
        }
    };

    nodes.sort_by(|a, b| a.private_ip.cmp(&b.private_ip));

    let head_public_ip = match nodes.first().and_then(|n| n.public_ip.as_deref()) {
        Some(ip) => ip.to_string(),
        None => {
            tracing::warn!("Head node has no public IP, cannot signal MPI job.");
            return;
        }
    };

    let ssh = SshSession::for_aws(&head_public_ip, private_key_path);
    match ssh.run_command("pkill -USR1 -f mpirun || true").await {
        Ok(_) => tracing::info!(
            "SIGUSR1 sent to MPI job on head node '{}'",
            head_public_ip
        ),
        Err(e) => tracing::warn!(
            "Failed to send SIGUSR1 to head node '{}': {}",
            head_public_ip,
            e
        ),
    }
}
