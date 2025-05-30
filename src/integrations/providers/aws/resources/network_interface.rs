use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_network_interface(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<String> {
        let context_subnet_id = context.subnet_id.as_ref().unwrap();
        let context_security_group_ids = &context.security_group_ids;

        if context_security_group_ids.is_empty() {
            bail!("Security group IDs are required but not set in context");
        }

        let eni_name = context.network_interface_name(node_index);
        let private_ip = context.network_interface_private_ip(node_index);

        // Check if ENI already exists
        let describe_eni_response = match context
            .client
            .describe_network_interfaces()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&eni_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!(
                    "Failed to describe network interface '{}': {:?}",
                    eni_name, e
                );
                bail!("Failure describing network interface resources");
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                info!(
                    "Found existing network interface '{}': '{}'",
                    eni_name, eni_id
                );

                // Verify ENI is in correct subnet
                if let Some(subnet_id) = eni.subnet_id() {
                    if subnet_id == context_subnet_id {
                        info!("Network interface '{}' is in correct subnet", eni_id);
                        return Ok(eni_id.to_string());
                    } else {
                        warn!(
                            "Network interface '{}' is in different subnet '{}', expected '{}'",
                            eni_id, subnet_id, context_subnet_id
                        );
                        bail!("Network interface is in wrong subnet");
                    }
                }
            }
        }

        info!(
            "Creating network interface '{}' for node {}...",
            eni_name, node_index
        );

        // Create ENI
        let mut create_request = context
            .client
            .create_network_interface()
            .subnet_id(context_subnet_id)
            .set_groups(Some(context_security_group_ids.clone()))
            .private_ip_address(&private_ip);

        // Add EFA interface type if node affinity is enabled
        if context.node_affinity {
            create_request = create_request
                .interface_type(aws_sdk_ec2::types::NetworkInterfaceCreationType::Efa);
        }

        // Add tags
        create_request = create_request.tag_specifications(
            aws_sdk_ec2::types::TagSpecification::builder()
                .resource_type(aws_sdk_ec2::types::ResourceType::NetworkInterface)
                .tags(
                    aws_sdk_ec2::types::Tag::builder()
                        .key("Name")
                        .value(&eni_name)
                        .build(),
                )
                .tags(context.cluster_id_tag.clone())
                .build(),
        );

        let create_eni_response = match create_request.send().await {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to create network interface '{}': {:?}", eni_name, e);
                bail!("Failure creating network interface resource");
            }
        };

        if let Some(eni_id) = create_eni_response
            .network_interface()
            .and_then(|eni| eni.network_interface_id())
        {
            info!(
                "Created network interface '{}' with ID '{}'{}",
                eni_name,
                eni_id,
                if context.node_affinity {
                    " (EFA enabled)"
                } else {
                    ""
                }
            );
            Ok(eni_id.to_string())
        } else {
            warn!(
                "Unexpected response when creating network interface '{}'",
                eni_name
            );
            bail!("Unexpected response from AWS when creating network interface");
        }
    }

    pub async fn cleanup_network_interface(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<()> {
        let eni_name = context.network_interface_name(node_index);

        // Find ENI by name tag
        let describe_eni_response = match context
            .client
            .describe_network_interfaces()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&eni_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing network interface resources");
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                info!(
                    "Found network interface to cleanup '{}': '{}'",
                    eni_name, eni_id
                );

                // Check if ENI is attached to an instance and detach if necessary
                if let Some(attachment) = eni.attachment() {
                    if let Some(instance_id) = attachment.instance_id() {
                        info!(
                            "Network interface '{}' is attached to instance '{}', detaching...",
                            eni_id, instance_id
                        );

                        if let Some(attachment_id) = attachment.attachment_id() {
                            match context
                                .client
                                .detach_network_interface()
                                .attachment_id(attachment_id)
                                .force(true) // Force detachment
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Successfully detached network interface '{}' from instance '{}'",
                                        eni_id, instance_id
                                    );

                                    // Wait a moment for detachment to complete
                                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to detach network interface '{}': {:?}",
                                        eni_id, e
                                    );
                                    warn!(
                                        "Continuing with deletion attempt despite detachment failure"
                                    );
                                }
                            }
                        }
                    }
                }

                // Delete the ENI
                info!("Deleting network interface '{}'...", eni_id);
                match context
                    .client
                    .delete_network_interface()
                    .network_interface_id(eni_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Network interface '{}' deleted successfully", eni_id);
                    }
                    Err(e) => {
                        error!("Failed to delete network interface '{}': {:?}", eni_id, e);
                        // Changed to bail instead of warn+continue since we're only handling one ENI
                        bail!("Failure deleting network interface resource");
                    }
                }
            }
        } else {
            info!("No network interface found for cleanup: '{}'", eni_name);
        }

        Ok(())
    }
}
