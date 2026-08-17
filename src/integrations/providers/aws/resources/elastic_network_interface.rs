use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;
use tokio::time::Duration;

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
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failed to describe Elastic Network Interface '{}'",
                    eni_name
                );
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                tracing::info!(
                    "Found existing Elastic Network Interface '{}': '{}'",
                    eni_name,
                    eni_id
                );
                return Ok(eni_id.to_string());
            }
        }

        tracing::info!(
            "Creating network interface '{}' for node {}...",
            eni_name,
            node_index
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failed to create Elastic Network Interface '{}'", eni_name);
            }
        };

        if let Some(eni_id) = create_eni_response
            .network_interface()
            .and_then(|eni| eni.network_interface_id())
        {
            tracing::info!(
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

        tracing::warn!("{:?}", create_eni_response);
        anyhow::bail!("Failure finding the id of the created Elastic Network Interface resource");
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
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failure describing Elastic Network Interface resources: {}",
                    e
                );
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                tracing::info!(
                    "Found Elastic Network Interface to cleanup '{}': '{}'",
                    eni_name,
                    eni_id
                );

                if let Some(attachment) = eni.attachment() {
                    if let Some(instance_id) = attachment.instance_id() {
                        tracing::info!(
                            "Elastic Network Interface '{}' is attached to Instance '{}', detaching...",
                            eni_id,
                            instance_id
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
                                    tracing::info!(
                                        "Successfully initiated detachment of Elastic Network Interface '{}' from Instance '{}'",
                                        eni_id,
                                        instance_id
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("{:?}", e);
                                    anyhow::bail!(
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

                tracing::info!("Deleting Elastic Network Interface '{}'...", eni_id);
                match context
                    .ec2_client
                    .delete_network_interface()
                    .network_interface_id(eni_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!(
                            "Elastic Network Interface '{}' deleted successfully",
                            eni_id
                        );
                    }
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failed to delete Elastic Network interface '{}'", eni_id);
                    }
                }
            }
        }

        tracing::info!("No Elastic Network Interface found");
        Ok(())
    }

    /// Delete any detached ENI whose private IP matches `private_ip`.
    /// Used by the scale-down path to clean up the dead node's ENI before it
    /// blocks security-group and subnet deletion on cluster termination.
    /// Blocks until the ENIs for `private_ips` have been released by the
    /// instances that held them, or until `max_wait` elapses.
    ///
    /// A replacement instance reuses the failed node's ENI, so it cannot be
    /// launched while the terminating instance still holds it. There is no way
    /// to shorten that wait, since the address only frees when the provider
    /// finishes reclaiming the instance, but there is no reason to guess at its
    /// duration either. Polling replaces a fixed sleep that was simultaneously
    /// too long on a fast detach and too short on a slow one, which is how a
    /// restore came to fail outright with InvalidNetworkInterface.InUse.
    ///
    /// Returns true if every address was released within the deadline.
    pub async fn wait_for_enis_released(
        &self,
        region: &str,
        private_ips: &[String],
        max_wait: std::time::Duration,
    ) -> bool {
        let ec2_client = match self.get_ec2_client(region).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Could not create EC2 client to poll ENIs: {}", e);
                return false;
            }
        };

        let started = std::time::Instant::now();
        loop {
            let mut all_free = true;
            for private_ip in private_ips {
                let attached = match ec2_client
                    .describe_network_interfaces()
                    .filters(
                        aws_sdk_ec2::types::Filter::builder()
                            .name("addresses.private-ip-address")
                            .values(private_ip)
                            .build(),
                    )
                    .send()
                    .await
                {
                    Ok(r) => r
                        .network_interfaces()
                        .iter()
                        .any(|eni| eni.status().map(|s| s.as_str()) != Some("available")),
                    // A failed describe is not evidence the ENI is free, so keep
                    // waiting rather than launching into a likely conflict.
                    Err(e) => {
                        tracing::debug!("Could not describe ENI for '{}': {}", private_ip, e);
                        true
                    }
                };
                if attached {
                    all_free = false;
                    break;
                }
            }

            if all_free {
                tracing::info!(
                    "Network interface(s) released after {:.0}s",
                    started.elapsed().as_secs_f64()
                );
                return true;
            }
            if started.elapsed() >= max_wait {
                tracing::warn!(
                    "Network interface(s) still attached after {:.0}s; proceeding anyway \
                     (the launch retry will absorb it if they are still busy)",
                    started.elapsed().as_secs_f64()
                );
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    pub async fn delete_detached_eni_by_private_ip(
        &self,
        region: &str,
        private_ip: &str,
    ) -> Result<()> {
        let ec2_client = self.get_ec2_client(region).await?;

        let response = match ec2_client
            .describe_network_interfaces()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("addresses.private-ip-address")
                    .values(private_ip)
                    .build(),
            )
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("status")
                    .values("available")
                    .build(),
            )
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "Could not describe ENIs for private IP '{}': {}",
                    private_ip,
                    e
                );
                return Ok(());
            }
        };

        for eni in response.network_interfaces() {
            if let Some(eni_id) = eni.network_interface_id() {
                match ec2_client
                    .delete_network_interface()
                    .network_interface_id(eni_id)
                    .send()
                    .await
                {
                    Ok(_) => tracing::info!(
                        "Deleted orphaned ENI '{}' for private IP '{}'",
                        eni_id,
                        private_ip
                    ),
                    Err(e) => tracing::warn!(
                        "Could not delete ENI '{}' for private IP '{}': {}",
                        eni_id,
                        private_ip,
                        e
                    ),
                }
            }
        }

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
                                tracing::info!(
                                    "Network interface '{}' reached desired status: {:?}",
                                    eni_id,
                                    desired_status
                                );
                                return Ok(());
                            }
                            Some(status) => {
                                tracing::info!(
                                    "Elastic Network Interface '{}' status: {:?}, waiting for {:?} (attempt {}/{})",
                                    eni_id,
                                    status,
                                    desired_status,
                                    attempt,
                                    max_attempts
                                );
                            }
                            None => {
                                tracing::warn!(
                                    "Elastic Network Interface '{}' status is unknown, waiting for {:?} (attempt {}/{})",
                                    eni_id,
                                    desired_status,
                                    attempt,
                                    max_attempts
                                );
                            }
                        }
                    } else {
                        anyhow::bail!(
                            "Elastic Network Interface '{}' not found during status check",
                            eni_id
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failed to check Elastic Network Interface '{}' status",
                        eni_id
                    );
                }
            }

            if attempt < max_attempts {
                tokio::time::sleep(sleep_duration).await;
            }
        }

        anyhow::bail!(
            "Elastic Network Interface '{}' did not reach desired status {:?} within {} seconds",
            eni_id,
            desired_status,
            max_attempts * sleep_duration.as_secs()
        );
    }
}
