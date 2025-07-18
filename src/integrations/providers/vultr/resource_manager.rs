use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;

use anyhow::{Result, bail};
use std::collections::HashMap;

use super::interface::VultrInterface;

impl CloudResourceManager for VultrInterface {
    async fn spawn_cluster(
        &self,
        _cluster: Cluster,
        _nodes: Vec<Node>,
        _init_commands: HashMap<usize, Vec<String>>,
    ) -> Result<()> {
        bail!("Not implemented")
    }

    async fn terminate_cluster(&self, _cluster: Cluster, _nodes: Vec<Node>) -> Result<()> {
        bail!("Not implemented")
    }

    async fn simulate_cluster_failure(
        &self,
        _cluster: Cluster,
        _node_private_ip: &str,
    ) -> Result<()> {
        bail!("Not implemented")
    }
}
