use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_subnet(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_subnets_response = match context
            .ec2_client
            .describe_subnets()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing Subnet resources");
            }
        };

        let subnets = describe_subnets_response.subnets();
        if let Some(subnet) = subnets.first() {
            if let Some(subnet_id) = subnet.subnet_id() {
                info!("Found existing Subnet: '{}'", subnet_id);
                return Ok(subnet_id.to_string());
            }
        }

        info!("No existing Subnet found, creating a new one...");

        let create_subnet_response = match context
            .ec2_client
            .create_subnet()
            .vpc_id(context.vpc_id.as_ref().unwrap())
            .cidr_block(context.subnet_cidr_block.clone())
            .availability_zone(context.availability_zone.clone())
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Subnet)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.subnet_name.clone())
                            .build(),
                    )
                    .tags(context.cluster_id_tag.clone())
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure creating Subnet resource");
            }
        };

        if let Some(subnet_id) = create_subnet_response
            .subnet()
            .and_then(|subnet| subnet.subnet_id())
        {
            info!("Created new Subnet '{}'", subnet_id);
            return Ok(subnet_id.to_string());
        }

        warn!("{:?}", create_subnet_response);
        bail!("Failure finding the id of the created Subnet resource");
    }

    pub async fn cleanup_subnet(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_subnets_response = match context
            .ec2_client
            .describe_subnets()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing subnet resources");
            }
        };

        let subnets = describe_subnets_response.subnets();
        if let Some(subnet) = subnets.first() {
            if let Some(subnet_id) = subnet.subnet_id() {
                info!("Found existing Subnet to cleanup: '{}'", subnet_id);
                info!("Deleting Subnet '{}'...", subnet_id);
                match context
                    .ec2_client
                    .delete_subnet()
                    .subnet_id(subnet_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Subnet '{}' deleted successfully", subnet_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure deleting Subnet resource");
                    }
                };
            }
        }

        info!("No existing Subnet found");
        Ok(())
    }
}
