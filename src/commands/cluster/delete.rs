use crate::database::models::{Cluster, ClusterState};

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use tracing::info;

pub async fn delete(pool: &SqlitePool, cluster_id: &str) -> Result<()> {
    let cluster = match Cluster::fetch_by_id(pool, cluster_id).await? {
        Some(cluster) => cluster,
        None => {
            println!("Cluster (id='{}') not found", cluster_id);
            return Ok(());
        }
    };

    match cluster.state {
        ClusterState::Pending | ClusterState::Terminated | ClusterState::Failed => {
            info!("Deleting Cluster '{}'...", cluster.display_name)
        }
        ClusterState::Spawning | ClusterState::Running | ClusterState::Terminating => {
            bail!(
                "Cannot delete Cluster '{}' in state '{}' from the DB",
                cluster.display_name,
                cluster.state
            );
        }
    }

    Cluster::delete(pool, cluster_id).await?;
    println!(
        "\nCluster '{}' (id='{}') is now deleted.",
        cluster.display_name, cluster.id
    );
    Ok(())
}
