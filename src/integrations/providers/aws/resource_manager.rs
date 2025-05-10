use crate::database::models::{Cluster, Node};
use crate::integrations::{CloudErrorHandler, CloudResourceManager};

use anyhow::{Error, Result};

use super::interface::AwsInterface;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        let client = match self.get_ec2_client(&cluster.region) {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize EC2 client"),
        };

        let vpc_cidr_block = "10.0.0.6/16";
        let _create_vpc_request = client
            .create_vpc()
            .cidr_block(vpc_cidr_block)
            .instance_tenancy(aws_sdk_ec2::types::Tenancy::Default)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(format!("cluster-{}-vpc", cluster.id))
                            .build(),
                    )
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("ClusterId")
                            .value(cluster.id.to_string())
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await;

        // Continue, after VPC, crete subnets, etc.. up to the nodes and security rules.

        Ok(())
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
