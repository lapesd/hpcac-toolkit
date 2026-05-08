use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;

impl AwsInterface {
    pub async fn ensure_vpc(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_vpcs_response = match context
            .ec2_client
            .describe_vpcs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing VPC resources: {}", e);
            }
        };

        let vpcs = describe_vpcs_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            if let Some(vpc_id) = vpc.vpc_id() {
                tracing::info!("Found existing VPC: '{}'", vpc_id);
                return Ok(vpc_id.to_string());
            }
        }

        tracing::info!("No existing VPC found, creating a new one...");

        let create_vpc_response = match context
            .ec2_client
            .create_vpc()
            .cidr_block(context.vpc_cidr_block.clone())
            .amazon_provided_ipv6_cidr_block(false)
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure creating VPC resource: {}", e);
            }
        };

        if let Some(vpc_id) = create_vpc_response.vpc().and_then(|vpc| vpc.vpc_id()) {
            tracing::info!("Created new VPC '{}'", vpc_id);
            match context
                .ec2_client
                .modify_vpc_attribute()
                .vpc_id(vpc_id)
                .enable_dns_hostnames(
                    aws_sdk_ec2::types::AttributeBooleanValue::builder()
                        .value(true)
                        .build(),
                )
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!("Enabled DNS hostnames for VPC (id='{}')", vpc_id);
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure enabling DNS hostnames for VPC (id='{}'): {}",
                        vpc_id,
                        e
                    );
                }
            };

            match context
                .ec2_client
                .modify_vpc_attribute()
                .vpc_id(vpc_id)
                .enable_dns_support(
                    aws_sdk_ec2::types::AttributeBooleanValue::builder()
                        .value(true)
                        .build(),
                )
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!("Enabled DNS support for VPC (id='{}')", vpc_id);
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure enabling DNS support for VPC (id='{}'): {}",
                        vpc_id,
                        e
                    );
                }
            }

            return Ok(vpc_id.to_string());
        }

        tracing::warn!("{:?}", create_vpc_response);
        anyhow::bail!("Failure finding the id of the created VPC resource");
    }

    pub async fn cleanup_vpc(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_vpcs_response = match context
            .ec2_client
            .describe_vpcs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing VPC resources: {}", e);
            }
        };

        let vpcs = describe_vpcs_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            if let Some(vpc_id) = vpc.vpc_id() {
                tracing::info!("Found existing VPC to cleanup: '{}'", vpc_id);
                tracing::info!("Deleting VPC '{}'...", vpc_id);
                match context.ec2_client.delete_vpc().vpc_id(vpc_id).send().await {
                    Ok(_) => {
                        tracing::info!("VPC '{}' deleted successfully", vpc_id);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failure deleting VPC resource: {}", e);
                    }
                };
            }
        }

        tracing::info!("No existing VPC found");
        Ok(())
    }
}
