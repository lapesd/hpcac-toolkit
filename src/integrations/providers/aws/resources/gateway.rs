use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_gateway(&self, context: &AwsClusterContext) -> Result<String> {
        let context_vpc_id = context.vpc_id.as_ref().unwrap();

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
                bail!("Failure describing gateway resources");
            }
        };

        let gateways = describe_gateways_response.internet_gateways();
        if let Some(gateway) = gateways.first() {
            if let Some(gateway_id) = gateway.internet_gateway_id() {
                info!("Found existing gateway: '{}'", gateway_id);

                if let Some(attachment) = gateway.attachments().first() {
                    if let Some(attached_vpc_id) = attachment.vpc_id() {
                        if attached_vpc_id == context_vpc_id {
                            info!(
                                "Gateway '{}' is already attached to VPC '{}'",
                                gateway_id, attached_vpc_id
                            );
                            return Ok(gateway_id.to_string());
                        } else {
                            error!(
                                "Gateway '{}' is attached to a different VPC '{}', expected '{}'",
                                gateway_id, attached_vpc_id, context_vpc_id
                            );
                            bail!("Failure attaching gateway to context VPC")
                        }
                    }
                }

                info!(
                    "Attaching existing gateway '{}' to VPC '{}'...",
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
                            "Successfully attached gateway '{}' to VPC '{}'",
                            gateway_id, context_vpc_id
                        );
                        return Ok(gateway_id.to_string());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure attaching gateway to context VPC")
                    }
                }
            }
        }

        info!("No existing gateway found, creating a new one...");

        let create_gateway_response = match context
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
                bail!("Failure creating gateway resource");
            }
        };

        if let Some(gateway_id) = create_gateway_response
            .internet_gateway()
            .and_then(|gateway| gateway.internet_gateway_id())
        {
            info!("Created new gateway '{}'", gateway_id);
            info!(
                "Attaching existing gateway '{}' to VPC '{}'...",
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
                        "Successfully attached gateway '{}' to VPC '{}'",
                        gateway_id, context_vpc_id
                    );
                    Ok(gateway_id.to_string())
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure attaching gateway to context VPC")
                }
            }
        } else {
            warn!("{:?}", create_gateway_response);
            bail!("Unexpected response from AWS when creating a gateway resource");
        }
    }

    pub async fn cleanup_gateway(&self, context: &AwsClusterContext) -> Result<()> {
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
                bail!("Failure describing gateway resources");
            }
        };

        let gateways = describe_gateways_response.internet_gateways();
        if let Some(gateway) = gateways.first() {
            if let Some(gateway_id) = gateway.internet_gateway_id() {
                info!("Found existing gateway to cleanup: '{}'", gateway_id);

                if let Some(attachment) = gateway.attachments().first() {
                    if let Some(attached_vpc_id) = attachment.vpc_id() {
                        info!(
                            "Detaching gateway '{}' from VPC '{}'...",
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
                                    "Successfully detached gateway '{}' from VPC '{}'",
                                    gateway_id, attached_vpc_id
                                );
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                bail!("Failure detaching gateway from VPC");
                            }
                        }
                    }
                }

                info!("Deleting gateway '{}'...", gateway_id);
                match context
                    .client
                    .delete_internet_gateway()
                    .internet_gateway_id(gateway_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Gateway '{}' deleted successfully", gateway_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure deleting gateway resource");
                    }
                }
            }
        }

        info!("No existing gateway found");
        Ok(())
    }
}
