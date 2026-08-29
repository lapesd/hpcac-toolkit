use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;

use super::interface::GcpInterface;

impl CloudResourceManager for GcpInterface {
    async fn spawn_cluster(
        &self,
        _pool: &SqlitePool,
        _cluster: Cluster,
        _nodes: Vec<Node>,
    ) -> Result<()> {
        bail!("Not implemented")
    }

    async fn terminate_cluster(
        &self,
        _pool: &SqlitePool,
        _cluster: Cluster,
        _nodes: Vec<Node>,
    ) -> Result<()> {
        bail!("Not implemented")
    }

    async fn simulate_cluster_failure(
        &self,
        _pool: &SqlitePool,
        _cluster: Cluster,
        _node_private_ip: &str,
    ) -> Result<()> {
        bail!("Not implemented")
    }
}
