use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tokio::time::Duration;
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_elastic_network_interface(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<String> {
        let eni_name = context.network_interface_name(node_index);
        let private_ip = context.network_interface_private_ip(node_index);
        let context_subnet_id = context.subnet_id.as_ref().unwrap();
        let context_security_group_ids = &context.security_group_ids;

        let describe_eni_response = match context
            .ec2_client
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
                bail!(
                    "Failed to describe Elastic Network Interface '{}'",
                    eni_name
                );
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                info!(
                    "Found existing Elastic Network Interface '{}': '{}'",
                    eni_name, eni_id
                );
                return Ok(eni_id.to_string());
            }
        }

        info!(
            "Creating network interface '{}' for node {}...",
            eni_name, node_index
        );

        let mut create_request = context
            .ec2_client
            .create_network_interface()
            .subnet_id(context_subnet_id)
            .set_groups(Some(context_security_group_ids.clone()))
            .private_ip_address(&private_ip);
        if context.use_elastic_fabric_adapters {
            create_request = create_request
                .interface_type(aws_sdk_ec2::types::NetworkInterfaceCreationType::Efa);
        }
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
                error!("{:?}", e);
                bail!("Failed to create Elastic Network Interface '{}'", eni_name);
            }
        };

        if let Some(eni_id) = create_eni_response
            .network_interface()
            .and_then(|eni| eni.network_interface_id())
        {
            info!(
                "Created new Elastic Network Interface '{}'{}",
                eni_id,
                if context.use_node_affinity {
                    " (EFA enabled)"
                } else {
                    ""
                }
            );
            return Ok(eni_id.to_string());
        }

        warn!("{:?}", create_eni_response);
        bail!("Failure finding the id of the created Elastic Network Interface resource");
    }

    pub async fn cleanup_elastic_network_interface(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<()> {
        let eni_name = context.network_interface_name(node_index);

        let describe_eni_response = match context
            .ec2_client
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
                bail!("Failure describing Elastic Network Interface resources");
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                info!(
                    "Found Elastic Network Interface to cleanup '{}': '{}'",
                    eni_name, eni_id
                );

                if let Some(attachment) = eni.attachment() {
                    if let Some(instance_id) = attachment.instance_id() {
                        info!(
                            "Elastic Network Interface '{}' is attached to Instance '{}', detaching...",
                            eni_id, instance_id
                        );
                        if let Some(attachment_id) = attachment.attachment_id() {
                            match context
                                .ec2_client
                                .detach_network_interface()
                                .attachment_id(attachment_id)
                                .force(true) // Force detachment
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Successfully initiated detachment of Elastic Network Interface '{}' from Instance '{}'",
                                        eni_id, instance_id
                                    );
                                }
                                Err(e) => {
                                    error!("{:?}", e);
                                    bail!(
                                        "Failed to detach Elastic Network Interface '{}'",
                                        eni_id
                                    );
                                }
                            }

                            self.wait_for_eni_status(
                                context,
                                eni_id,
                                aws_sdk_ec2::types::NetworkInterfaceStatus::Available,
                            )
                            .await?;
                        }
                    }
                }

                info!("Deleting Elastic Network Interface '{}'...", eni_id);
                match context
                    .ec2_client
                    .delete_network_interface()
                    .network_interface_id(eni_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!(
                            "Elastic Network Interface '{}' deleted successfully",
                            eni_id
                        );
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failed to delete Elastic Network interface '{}'", eni_id);
                    }
                }
            }
        }

        info!("No Elastic Network Interface found");
        Ok(())
    }

    async fn wait_for_eni_status(
        &self,
        context: &AwsClusterContext,
        eni_id: &str,
        desired_status: aws_sdk_ec2::types::NetworkInterfaceStatus,
    ) -> Result<()> {
        let max_attempts = 10; // Maximum number of attempts (10 * 6 seconds = 1 minute)
        let sleep_duration = Duration::from_secs(6);

        for attempt in 1..=max_attempts {
            match context
                .ec2_client
                .describe_network_interfaces()
                .network_interface_ids(eni_id)
                .send()
                .await
            {
                Ok(response) => {
                    if let Some(eni) = response.network_interfaces().first() {
                        match eni.status() {
                            Some(status) if *status == desired_status => {
                                info!(
                                    "Network interface '{}' reached desired status: {:?}",
                                    eni_id, desired_status
                                );
                                return Ok(());
                            }
                            Some(status) => {
                                info!(
                                    "Elastic Network Interface '{}' status: {:?}, waiting for {:?} (attempt {}/{})",
                                    eni_id, status, desired_status, attempt, max_attempts
                                );
                            }
                            None => {
                                warn!(
                                    "Elastic Network Interface '{}' status is unknown, waiting for {:?} (attempt {}/{})",
                                    eni_id, desired_status, attempt, max_attempts
                                );
                            }
                        }
                    } else {
                        bail!(
                            "Elastic Network Interface '{}' not found during status check",
                            eni_id
                        );
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!(
                        "Failed to check Elastic Network Interface '{}' status",
                        eni_id
                    );
                }
            }

            if attempt < max_attempts {
                tokio::time::sleep(sleep_duration).await;
            }
        }

        bail!(
            "Elastic Network Interface '{}' did not reach desired status {:?} within {} seconds",
            eni_id,
            desired_status,
            max_attempts * sleep_duration.as_secs()
        );
    }
}
