use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;

use anyhow::Result;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let mut context = self.create_cluster_context(&cluster)?;

        context.vpc_id = Some(self.ensure_vpc(&context).await?);
        context.subnet_id = Some(self.ensure_subnet(&context).await?);
        context.gateway_id = Some(self.ensure_gateway(&context).await?);
        context.route_table_id = Some(self.ensure_route_table(&context).await?);
        context.security_group_ids = self.ensure_security_group(&context).await?;
        context.ssh_key_id = Some(self.ensure_ssh_key(&context).await?);

        if context.node_affinity {
            context.placement_group_name_actual =
                Some(self.ensure_placement_group(&context).await?);
        }

        for (node_index, _node) in nodes.iter().enumerate() {
            self.ensure_network_interface(&context, node_index).await?;
        }

        Ok(())
    }

    async fn destroy_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let context = self.create_cluster_context(&cluster)?;

        for (node_index, _node) in nodes.iter().enumerate() {
            self.cleanup_network_interface(&context, node_index).await?;
        }

        if context.node_affinity {
            self.cleanup_placement_group(&context).await?;
        }

        self.cleanup_ssh_key(&context).await?;
        self.cleanup_security_group(&context).await?;
        self.cleanup_route_table(&context).await?;
        self.cleanup_gateway(&context).await?;
        self.cleanup_subnet(&context).await?;
        self.cleanup_vpc(&context).await?;

        Ok(())
    }
}
