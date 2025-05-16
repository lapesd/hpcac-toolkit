use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node};
use crate::integrations::{CloudErrorHandler, CloudResourceManager};

use anyhow::{Error, Result, anyhow};
use tracing::info;

impl AwsInterface {
    async fn get_existing_vpc(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let vpc_query_response = match client
            .describe_vpcs()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return self.handle_error(e.into(), "Failed to query existing VPCs"),
        };
        let vpcs = vpc_query_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            return Ok(vpc.vpc_id().map(String::from));
        }
        Ok(None)
    }

    async fn create_vpc(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_name: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Creating a new VPC...");
        let vpc_cidr_block = "10.0.0.0/16";
        let vpc_output = match client
            .create_vpc()
            .cidr_block(vpc_cidr_block)
            // TODO: Evaluate the possibility of using Dedicated tenancy
            .instance_tenancy(aws_sdk_ec2::types::Tenancy::Default)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(vpc_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating VPC"),
        };
        let vpc_id = match vpc_output.vpc().and_then(|vpc| vpc.vpc_id()) {
            Some(id) => {
                info!("Created new VPC '{}'", id);
                id
            }
            None => {
                let error_msg = "Missing VPC id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };
        Ok(vpc_id.to_string())
    }

    async fn get_existing_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let subnet_query_response = match client
            .describe_subnets()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return self.handle_error(e.into(), "Failed to query existing subnets"),
        };

        let subnets = subnet_query_response.subnets();
        if let Some(subnet) = subnets.first() {
            return Ok(subnet.subnet_id().map(String::from));
        }

        Ok(None)
    }

    // New helper method to create a subnet
    async fn create_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_id: &str,
        subnet_name: &str,
        availability_zone: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!(
            "Creating a new subnet in availability zone '{}'...",
            availability_zone
        );
        let subnet_cidr_block = "10.0.1.0/24";
        let subnet_output = match client
            .create_subnet()
            .vpc_id(vpc_id)
            .cidr_block(subnet_cidr_block)
            .availability_zone(availability_zone)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Subnet)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(subnet_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating subnet"),
        };

        let subnet_id = match subnet_output.subnet().and_then(|subnet| subnet.subnet_id()) {
            Some(id) => {
                info!("Created new subnet '{}' in AZ '{}'", id, availability_zone);
                id
            }
            None => {
                let error_msg = "Missing subnet id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(subnet_id.to_string())
    }

    async fn delete_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        subnet_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting subnet '{}'...", subnet_id);
        match client.delete_subnet().subnet_id(subnet_id).send().await {
            Ok(_) => {
                info!("Subnet '{}' deleted successfully", subnet_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete subnet '{}'", subnet_id),
            ),
        }
    }

    async fn delete_vpc(&self, client: &aws_sdk_ec2::Client, vpc_id: &str) -> Result<(), Error> {
        info!("Deleting VPC '{}'...", vpc_id);
        match client.delete_vpc().vpc_id(vpc_id).send().await {
            Ok(_) => {
                info!("VPC '{}' deleted successfully", vpc_id);
                Ok(())
            }
            Err(e) => self.handle_error(e.into(), &format!("Failed to delete VPC '{}'", vpc_id)),
        }
    }
}

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        // Get AWS SDK client
        let client = self.get_ec2_client(&cluster.region)?;

        let cluster_tag = aws_sdk_ec2::types::Tag::builder()
            .key("ClusterId")
            .value(cluster.id.to_string())
            .build();

        // Create VPC
        let vpc_name = format!("HPC@CLOUD-{}-VPC", cluster.id);
        let vpc_id = match self.get_existing_vpc(&client, &cluster.id).await {
            Ok(Some(id)) => {
                info!("VPC for this cluster already exists, skipping creation...");
                id
            }
            Ok(None) => {
                self.create_vpc(&client, &vpc_name, cluster_tag.clone())
                    .await?
            }
            Err(e) => return self.handle_error(e, "Failed to check for existing VPC"),
        };

        // Create SUBNET
        let subnet_name = format!("HPC@CLOUD-{}-Subnet", cluster.id);
        let _subnet_id = match self.get_existing_subnet(&client, &cluster.id).await {
            Ok(Some(id)) => {
                info!("Subnet for this cluster already exists, skipping creation...");
                id
            }
            Ok(None) => {
                self.create_subnet(
                    &client,
                    &vpc_id,
                    &subnet_name,
                    &cluster.availability_zone,
                    cluster_tag.clone(),
                )
                .await?
            }
            Err(e) => return self.handle_error(e, "Failed to check for existing subnet"),
        };

        // TODO: Continue spawning the Cluster
        Ok(())
    }

    async fn destroy_cluster(&self, cluster: Cluster) -> Result<(), Error> {
        // Get AWS SDK client
        let client = self.get_ec2_client(&cluster.region)?;

        // Delete SUBNET
        match self.get_existing_subnet(&client, &cluster.id).await {
            Ok(Some(subnet_id)) => {
                info!("Found subnet '{}' for cluster, deleting...", subnet_id);
                self.delete_subnet(&client, &subnet_id).await?;
            }
            Ok(None) => {
                info!("No subnet found for cluster {}", cluster.id);
            }
            Err(e) => {
                return self.handle_error(e, "Failed finding cluster Subnet");
            }
        }

        // Delete VPC
        match self.get_existing_vpc(&client, &cluster.id).await {
            Ok(Some(vpc_id)) => {
                info!("Found VPC '{}' for cluster, deleting...", vpc_id);
                self.delete_vpc(&client, &vpc_id).await?;
            }
            Ok(None) => {
                info!("No VPC found for cluster {}", cluster.id);
            }
            Err(e) => {
                return self.handle_error(e, "Failed finding cluster VPC");
            }
        }

        // TODO: Continue destroying the Cluster
        Ok(())
    }
}
