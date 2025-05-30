use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_vpc(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_vpcs_response = match context
            .client
            .describe_vpcs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing VPC resources");
            }
        };

        let vpcs = describe_vpcs_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            if let Some(vpc_id) = vpc.vpc_id() {
                info!("Found existing VPC: '{}'", vpc_id);
                return Ok(vpc_id.to_string());
            }
        }

        info!("No existing VPC found, creating a new one...");

        let create_vpc_response = match context
            .client
            .create_vpc()
            .cidr_block(context.vpc_cidr_block.clone())
            // TODO: Evaluate the possibility of using Dedicated tenancy
            .instance_tenancy(aws_sdk_ec2::types::Tenancy::Default)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.vpc_name.clone())
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
                bail!("Failure creating VPC resource");
            }
        };

        if let Some(vpc_id) = create_vpc_response.vpc().and_then(|vpc| vpc.vpc_id()) {
            info!("Created new VPC '{}'", vpc_id);
            return Ok(vpc_id.to_string());
        }

        warn!("{:?}", create_vpc_response);
        bail!("Unexpected response from AWS when creating a VPC resource");
    }

    pub async fn cleanup_vpc(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_vpcs_response = match context
            .client
            .describe_vpcs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing VPC resources");
            }
        };

        let vpcs = describe_vpcs_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            if let Some(vpc_id) = vpc.vpc_id() {
                info!("Found existing VPC to cleanup: '{}'", vpc_id);
                info!("Deleting VPC '{}'...", vpc_id);
                match context.client.delete_vpc().vpc_id(vpc_id).send().await {
                    Ok(_) => {
                        info!("VPC '{}' deleted successfully", vpc_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure deleting VPC resource");
                    }
                };
            }
        }

        info!("No existing VPC found");
        Ok(())
    }
}
