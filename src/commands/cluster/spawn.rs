use crate::database::models::{Cluster, ProviderConfig};
use crate::integrations::{
    cloud_interface::{CloudInfoProvider, CloudResourceManager},
    providers::aws::AwsInterface,
};
use crate::utils;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

pub async fn spawn(pool: &SqlitePool, cluster_id: &str, skip_confirmation: bool, retry: u32) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            anyhow::bail!("Cluster (id='{}') not found", cluster_id);
        }
    };

    let provider_config =
        match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
            Some(config) => config,
            None => {
                anyhow::bail!(
                    "ProviderConfig (id='{}') not found",
                    cluster.provider_config_id
                );
            }
        };

    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => AwsInterface { config_vars },
        _ => {
            anyhow::bail!(
                "Provider (id='{}') is currently not supported.",
                &provider_id
            )
        }
    };

    match cluster.state {
        crate::database::models::ClusterState::Running
        | crate::database::models::ClusterState::Terminating => {
            anyhow::bail!(
                "Cluster '{}' is currently {}. Run 'cluster terminate' first.",
                cluster.display_name,
                cluster.state
            );
        }
        _ => {}
    }

    let nodes = cluster.get_nodes(pool).await?;
    cluster.print_details(pool).await?;

    // Fetch live pricing for cost estimate
    let unique_instance_types: Vec<String> = nodes
        .iter()
        .map(|n| n.instance_type.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let pricing_tracker = utils::ProgressTracker::new(
        unique_instance_types.len() as u64,
        Some("instance pricing"),
    );
    let live_instance_prices = cloud_interface
        .fetch_prices(&cluster.region, &unique_instance_types, &pricing_tracker)
        .await?;
    pricing_tracker.finish_with_message("Instance pricing fetched");

    let spot_instance_types: Vec<String> = nodes
        .iter()
        .filter(|n| n.allocation_mode == "spot")
        .map(|n| n.instance_type.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let live_spot_prices = if !spot_instance_types.is_empty() {
        cloud_interface
            .fetch_spot_prices(&cluster.region, &spot_instance_types, &cluster.availability_zone)
            .await?
    } else {
        std::collections::HashMap::new()
    };

    let unique_volume_types: std::collections::HashSet<String> =
        nodes.iter().map(|n| n.root_volume_type.clone()).collect();
    let mut ebs_price_cache = std::collections::HashMap::new();
    for vt in &unique_volume_types {
        ebs_price_cache.insert(
            vt.clone(),
            cloud_interface.fetch_ebs_pricing(&cluster.region, vt).await?,
        );
    }

    const GP3_THROUGHPUT_BASELINE_MBS: f64 = 125.0;
    const GP3_PROVISIONED_THROUGHPUT_MBS: f64 = 500.0;
    const GP3_IOPS_BASELINE: f64 = 3000.0;
    const GP3_IOPS_PER_MONTH: f64 = 0.005;
    const IO1_IO2_IOPS_PER_MONTH: f64 = 0.065;
    const PUBLIC_IPV4_PER_HOUR: f64 = 0.005;
    const HOURS_PER_MONTH: f64 = 730.0;

    let live_cost_per_hour: f64 = nodes.iter().map(|node| {
        let instance_cost = if node.allocation_mode == "spot" {
            *live_spot_prices.get(&node.instance_type).unwrap_or(
                live_instance_prices.get(&node.instance_type).unwrap_or(&0.0),
            )
        } else {
            *live_instance_prices.get(&node.instance_type).unwrap_or(&0.0)
        };
        let ebs = ebs_price_cache.get(&node.root_volume_type).unwrap();
        let storage_cost = node.root_volume_gb as f64 * ebs.storage_per_gb_month / HOURS_PER_MONTH;
        let extra_throughput = (GP3_PROVISIONED_THROUGHPUT_MBS - GP3_THROUGHPUT_BASELINE_MBS).max(0.0);
        let throughput_cost = extra_throughput * ebs.throughput_per_mbs_month / HOURS_PER_MONTH;
        let iops_cost = match (node.root_volume_iops, node.root_volume_type.as_str()) {
            (Some(iops), "gp3") => (iops as f64 - GP3_IOPS_BASELINE).max(0.0) * GP3_IOPS_PER_MONTH / HOURS_PER_MONTH,
            (Some(iops), "io1") | (Some(iops), "io2") => iops as f64 * IO1_IO2_IOPS_PER_MONTH / HOURS_PER_MONTH,
            _ => 0.0,
        };
        instance_cost + storage_cost + throughput_cost + iops_cost + PUBLIC_IPV4_PER_HOUR
    }).sum();

    tracing::info!(
        "Estimated cost: ${:.4}/hour (${:.2}/day)",
        live_cost_per_hour,
        live_cost_per_hour * 24.0
    );

    if !utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed spawning this cluster?",
    )? {
        return Ok(());
    }

    let mut attempt = 0u32;
    loop {
        // Re-fetch cluster and nodes each attempt: spawn_cluster takes ownership
        // and the EFS/VPC/subnet creation is idempotent on retry.
        let cluster = Cluster::fetch_by_id(pool, cluster_id).await?.unwrap();
        let nodes = cluster.get_nodes(pool).await?;

        match cloud_interface.spawn_cluster(pool, cluster, nodes).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                let msg = e.to_string();
                // Transient conditions worth another pass. Re-running spawn is cheap
                // because every resource step is idempotent, so a retry skips
                // straight back to whatever failed.
                //
                // The SSH cases matter as much as the capacity ones: sshd on a fresh
                // instance is not ready the moment the API reports the instance as
                // running, and a worker reached through the head node can time out
                // during banner exchange while it is still coming up. Those are
                // connection-level failures, deliberately distinguished from a
                // command that ran and returned non-zero, which must not be retried.
                let transient = msg.contains("InsufficientInstanceCapacity")
                    || msg.contains("InvalidNetworkInterface.InUse")
                    || msg.contains("banner exchange")
                    || msg.contains("Connection timed out")
                    || msg.contains("Connection refused")
                    || msg.contains("Connection reset");
                if attempt < retry && transient {
                    attempt += 1;
                    tracing::warn!(
                        "Transient spawn failure ({}) — retrying in 60s (attempt {}/{})...",
                        msg.lines().next().unwrap_or("unknown"),
                        attempt,
                        retry
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
}
