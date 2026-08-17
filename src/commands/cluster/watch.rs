use crate::commands::cluster::tasks::tasks;
use crate::database::models::{
    Cluster, ClusterState, Node, ProviderConfig, RecoveryNode, ShellCommand,
};
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
                    match Node::fetch_by_private_ip(&pool, &cluster.id, ip).await? {
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

                // A replacement reuses the failed node's network interface, so it
                // cannot launch until the terminating instance releases it. Poll
                // for that rather than sleeping a fixed interval: the detach is
                // not bounded, so any constant is both wasteful when it completes
                // quickly and insufficient when it does not.
                //
                // This is also the only part of recovery an interruption notice
                // could plausibly shorten, and it cannot. The address is held
                // until the provider finishes reclaiming the instance, which by
                // definition has not happened while the warning window is open.
                tracing::info!(
                    "[{}] Waiting for failed instance(s) to release network interfaces...",
                    Utc::now().format("%H:%M:%S")
                );
                cloud_interface
                    .wait_for_enis_released(&cluster.region, &failed_ips, Duration::from_secs(180))
                    .await;

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
                        if let Err(e) = Node::delete_by_private_ip(&pool, &cluster.id, ip).await {
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
///
/// If a recovery slot specifies `count > 1`, additional Node rows are inserted with
/// the same replacement spec, expanding the cluster on restart (scale-up recovery).
/// The additional rows are created with empty private_ip/public_ip; they will be
/// assigned by spawn_cluster during ENI provisioning.
///
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

    // Snapshot node identity we need across the loop (avoids borrow issues when we
    // later start a transaction to insert additional Node rows).
    #[derive(Clone)]
    struct NodeSlot {
        id: String,
        cluster_id: String,
        private_ip: Option<String>,
    }
    let node_slots: Vec<NodeSlot> = nodes
        .iter()
        .map(|n| NodeSlot {
            id: n.id.clone(),
            cluster_id: n.cluster_id.clone(),
            private_ip: n.private_ip.clone(),
        })
        .collect();

    // Fan-out entries deferred until after in-place updates so the read-only slot
    // scan sees a stable node list.
    struct FanOut<'a> {
        slot_index: usize,
        extra: i64, // number of ADDITIONAL nodes beyond the 1:1 replacement
        recovery: &'a RecoveryNode,
        instance_type: String,
        // Already resolved: either the slot's declared commands, or the ones
        // inherited from the node this slot fans out from.
        init_commands: Vec<String>,
    }
    let mut fanouts: Vec<FanOut> = Vec::new();
    // (node_id, commands) for in-place replacements whose slot declares its own.
    let mut init_overrides: Vec<(String, Vec<String>)> = Vec::new();

    for (slot_index, node) in node_slots.iter().enumerate() {
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
        // The spec written below is the preferred type. Any remaining entries are
        // capacity fallbacks, applied by spawn_cluster at launch time if AWS has
        // no capacity for the preferred one.
        let fallbacks: Vec<String> = recovery.instance_types().into_iter().skip(1).collect();
        tracing::info!(
            "Applying recovery policy to slot {} (ip='{}'): {} / {} (count={}){}",
            slot_index,
            ip,
            instance_type,
            recovery.allocation_mode,
            recovery.count,
            if fallbacks.is_empty() {
                ", no capacity fallback".to_string()
            } else {
                format!(", capacity fallbacks: {}", fallbacks.join(" -> "))
            }
        );

        // In-place: update the failed slot's original Node row to the new spec.
        // spawn_cluster respawns this node with the updated spec.
        let target = nodes
            .iter()
            .find(|n| n.id == node.id)
            .expect("slot id present in nodes");
        target
            .update_instance_spec(
                pool,
                &instance_type,
                &recovery.allocation_mode,
                &recovery.image_id,
                recovery.burstable_mode.as_deref(),
                recovery.root_volume_gb,
                &recovery.root_volume_type,
                recovery.root_volume_iops,
            )
            .await?;

        // A replacement reuses the failed node's row, so it would otherwise inherit
        // init commands written for the ORIGINAL hardware. That is right for an
        // in-kind swap and wrong across families — a GPU stand-in has a local NVMe
        // store to mount that the CPU node it replaces never had. A slot that
        // declares its own commands overrides them; one that stays silent inherits,
        // which is the pre-existing behaviour.
        let declared = recovery.declared_init_commands();
        if let Some(commands) = &declared {
            tracing::info!(
                "Slot {} declares {} init command(s) for its replacement",
                slot_index,
                commands.len()
            );
            init_overrides.push((node.id.clone(), commands.clone()));
        }

        // Fan-out: queue (count - 1) additional Node rows for this slot.
        if recovery.count > 1 {
            // Scale-up nodes are new rows with no commands of their own. Give them
            // the slot's declared list, or failing that a copy of the sibling they
            // fan out from, so an expanded cluster stays homogeneous.
            let init_commands = match &declared {
                Some(commands) => commands.clone(),
                None => {
                    let mut existing =
                        ShellCommand::fetch_all_by_node_id(pool, node.id.clone()).await?;
                    existing.sort_by_key(|c| c.ordering);
                    existing.into_iter().map(|c| c.script).collect()
                }
            };
            fanouts.push(FanOut {
                slot_index,
                extra: recovery.count - 1,
                recovery,
                instance_type,
                init_commands,
            });
        }
    }

    // Apply in-place overrides before the fan-out transaction so a slot's declared
    // commands land on the replacement even when count == 1.
    if !init_overrides.is_empty() {
        let mut tx = pool.begin().await?;
        for (node_id, commands) in &init_overrides {
            ShellCommand::replace_all_for_node(&mut tx, node_id, commands).await?;
        }
        tx.commit().await?;
        tracing::info!(
            "Recovery: replaced init commands on {} node(s)",
            init_overrides.len()
        );
    }

    // Materialize scale-up rows in a single transaction so partial failures roll back.
    if !fanouts.is_empty() {
        let mut tx = pool.begin().await?;
        let mut total_added = 0i64;
        for fo in &fanouts {
            for _ in 0..fo.extra {
                let new_node = Node {
                    id: utils::generate_id(),
                    cluster_id: cluster.id.clone(),
                    instance_type: fo.instance_type.clone(),
                    allocation_mode: fo.recovery.allocation_mode.clone(),
                    burstable_mode: fo.recovery.burstable_mode.clone(),
                    image_id: fo.recovery.image_id.clone(),
                    root_volume_gb: fo.recovery.root_volume_gb,
                    root_volume_type: fo.recovery.root_volume_type.clone(),
                    root_volume_iops: fo.recovery.root_volume_iops,
                    private_ip: None,
                    public_ip: None,
                    was_efs_configured: false,
                    was_ssh_configured: false,
                };
                new_node.insert(&mut tx).await?;
                if !fo.init_commands.is_empty() {
                    ShellCommand::replace_all_for_node(&mut tx, &new_node.id, &fo.init_commands)
                        .await?;
                }
                total_added += 1;
            }
            tracing::info!(
                "Scale-up: slot {} expanded by {} extra node(s) → spec {} / {}",
                fo.slot_index,
                fo.extra,
                fo.instance_type,
                fo.recovery.allocation_mode
            );
        }
        tx.commit().await?;
        tracing::info!(
            "Scale-up: inserted {} additional Node row(s) for spawn",
            total_added
        );
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

    // Signal the application ranks directly on every node, rather than the
    // launcher on the head node.
    //
    // The previous approach ran `pkill -USR1 -f mpirun` on the head and relied on
    // the MPI launcher forwarding the signal to its ranks. Whether it does is
    // implementation and version dependent (PRRTE exposes an explicit
    // --forward-signals option, so forwarding is not something to count on), and
    // the `|| true` meant a pkill that matched nothing still reported success.
    // The result was a preemptive checkpoint path that logged "signal sent" on
    // every interruption while the application never saw SIGUSR1 and never
    // flushed.
    //
    // Signalling the ranks removes the launcher from the path entirely. The
    // application's check is collective, so reaching any one rank is sufficient,
    // but we signal all of them because a node may be reclaimed mid-sweep.
    const SIGNAL_CMD: &str = "pkill -USR1 -x starfwi-fwi";

    let mut signalled = 0usize;
    let mut unreachable = 0usize;
    for (index, node) in nodes.iter().enumerate() {
        let Some(private_ip) = node.private_ip.as_deref() else {
            continue;
        };
        let ssh = if index == 0 {
            SshSession::for_aws(&head_public_ip, private_key_path)
        } else {
            SshSession::for_aws_worker(private_ip, &head_public_ip, private_key_path)
        };
        // pkill exits 1 when nothing matched, which is a real outcome worth
        // distinguishing from a node we could not reach at all.
        match ssh.run_command(SIGNAL_CMD).await {
            Ok(_) => {
                signalled += 1;
                tracing::debug!("SIGUSR1 delivered to ranks on '{}'", private_ip);
            }
            Err(e) => {
                unreachable += 1;
                tracing::debug!("Could not signal ranks on '{}': {}", private_ip, e);
            }
        }
    }

    if signalled > 0 {
        tracing::info!(
            "SIGUSR1 delivered to application ranks on {} of {} node(s)",
            signalled,
            nodes.len()
        );
    } else {
        tracing::warn!(
            "Preemptive checkpoint signal reached no ranks ({} node(s) unreachable or no matching process). \
             The job will not flush before reclamation and will resume from the last periodic checkpoint.",
            unreachable
        );
    }
}
