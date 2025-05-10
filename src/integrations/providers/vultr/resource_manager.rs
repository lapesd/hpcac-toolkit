use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;
use anyhow::{Error, Result};

use super::interface::VultrInterface;

impl CloudResourceManager for VultrInterface {
    async fn spawn_cluster(&self, _cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        anyhow::bail!("Not implemented")
    }

    async fn check_cluster_exists(&self, _cluster_id: &str) -> Result<bool, Error> {
        anyhow::bail!("Not implemented")
    }

    async fn delete_cluster(&self, _cluster_id: &str) -> Result<(), Error> {
        anyhow::bail!("Not implemented")
    }

    async fn cleanup_orphaned_resources(&self, _cluster_id: &str) -> Result<Vec<String>, Error> {
        anyhow::bail!("Not implemented")
    }
}
