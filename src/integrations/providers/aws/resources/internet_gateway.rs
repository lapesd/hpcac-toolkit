use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_internet_gateway(&self, context: &AwsClusterContext) -> Result<String> {
        let context_vpc_id = context.vpc_id.as_ref().unwrap();
        let describe_internet_gateways_response = match context
            .client
            .describe_internet_gateways()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing Internet Gateway resources");
            }
        };

        let gateways = describe_internet_gateways_response.internet_gateways();
        if let Some(gateway) = gateways.first() {
            if let Some(gateway_id) = gateway.internet_gateway_id() {
                info!("Found existing Internet Gateway: '{}'", gateway_id);

                if let Some(attachment) = gateway.attachments().first() {
                    if let Some(attached_vpc_id) = attachment.vpc_id() {
                        if attached_vpc_id == context_vpc_id {
                            info!(
                                "Internet Gateway '{}' is already attached to VPC '{}'",
                                gateway_id, attached_vpc_id
                            );
                            return Ok(gateway_id.to_string());
                        } else {
                            error!(
                                "Internet Gateway '{}' is attached to a different VPC '{}', expected '{}'",
                                gateway_id, attached_vpc_id, context_vpc_id
                            );
                            bail!("Failure attaching Internet Gateway to context VPC")
                        }
                    }
                }

                info!(
                    "Attaching existing Internet Gateway '{}' to VPC '{}'...",
                    gateway_id, context_vpc_id
                );
                match context
                    .client
                    .attach_internet_gateway()
                    .internet_gateway_id(gateway_id)
                    .vpc_id(context_vpc_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!(
                            "Successfully attached Internet Gateway '{}' to VPC '{}'",
                            gateway_id, context_vpc_id
                        );
                        return Ok(gateway_id.to_string());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure attaching Internet Gateway to context VPC")
                    }
                }
            }
        }

        info!("No existing Internet Gateway found, creating a new one...");

        let create_internet_gateway_response = match context
            .client
            .create_internet_gateway()
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::InternetGateway)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.gateway_name.clone())
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
                bail!("Failure creating Internet Gateway resource");
            }
        };

        if let Some(gateway_id) = create_internet_gateway_response
            .internet_gateway()
            .and_then(|gateway| gateway.internet_gateway_id())
        {
            info!("Created new Internet Gateway '{}'", gateway_id);
            info!(
                "Attaching existing Internet Gateway '{}' to VPC '{}'...",
                gateway_id, context_vpc_id
            );
            match context
                .client
                .attach_internet_gateway()
                .internet_gateway_id(gateway_id)
                .vpc_id(context_vpc_id)
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully attached Internet Gateway '{}' to VPC '{}'",
                        gateway_id, context_vpc_id
                    );
                    Ok(gateway_id.to_string())
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure attaching Internet Gateway to context VPC")
                }
            }
        } else {
            warn!("{:?}", create_internet_gateway_response);
            bail!("Failure finding the id of the created Internet Gateway resource");
        }
    }

    pub async fn cleanup_internet_gateway(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_gateways_response = match context
            .client
            .describe_internet_gateways()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing Internet Gateway resources");
            }
        };

        let gateways = describe_gateways_response.internet_gateways();
        if let Some(gateway) = gateways.first() {
            if let Some(gateway_id) = gateway.internet_gateway_id() {
                info!(
                    "Found existing Internet Gateway to cleanup: '{}'",
                    gateway_id
                );

                if let Some(attachment) = gateway.attachments().first() {
                    if let Some(attached_vpc_id) = attachment.vpc_id() {
                        info!(
                            "Detaching Internet Gateway '{}' from VPC '{}'...",
                            gateway_id, attached_vpc_id
                        );
                        match context
                            .client
                            .detach_internet_gateway()
                            .internet_gateway_id(gateway_id)
                            .vpc_id(attached_vpc_id)
                            .send()
                            .await
                        {
                            Ok(_) => {
                                info!(
                                    "Successfully detached Internet Gateway '{}' from VPC '{}'",
                                    gateway_id, attached_vpc_id
                                );
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                bail!("Failure detaching Internet Gateway from VPC");
                            }
                        }
                    }
                }

                info!("Deleting Internet Gateway '{}'...", gateway_id);
                match context
                    .client
                    .delete_internet_gateway()
                    .internet_gateway_id(gateway_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Internet Gateway '{}' deleted successfully", gateway_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure deleting Internet Gateway resource");
                    }
                }
            }
        }

        info!("No existing Internet Gateway found");
        Ok(())
    }
}
