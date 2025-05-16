use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;

use anyhow::{Error, Result};

use super::interface::VultrInterface;

impl CloudResourceManager for VultrInterface {
    async fn spawn_cluster(&self, _cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        anyhow::bail!("Not implemented")
    }

    async fn destroy_cluster(&self, _cluster: Cluster) -> Result<(), Error> {
        anyhow::bail!("Not implemented")
    }
}
