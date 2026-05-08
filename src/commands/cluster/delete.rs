use crate::database::models::{Cluster, ClusterState};

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

pub async fn delete(pool: &SqlitePool, cluster_id: &str) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            tracing::warn!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    match cluster.state {
        ClusterState::Pending
        | ClusterState::Terminated
        | ClusterState::Failed
        | ClusterState::Restoring => {
            tracing::info!("Deleting Cluster '{}'...", cluster.display_name)
        }
        _ => {
            anyhow::bail!(
                "Cannot delete Cluster '{}' in state '{}' from the DB",
                cluster.display_name,
                cluster.state
            );
        }
    }

    Cluster::delete(pool, cluster_id).await?;
    tracing::info!(
        "Cluster '{}' (id='{}') is now deleted.",
        cluster.display_name,
        cluster.id
    );
    Ok(())
}
