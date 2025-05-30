use crate::database::models::Node;
use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_ec2_instance(
        &self,
        context: &AwsClusterContext,
        node: &Node,
        node_index: usize,
    ) -> Result<String> {
        let instance_name = context.ec2_instance_name(node_index);

        // Check if instance for this node already exists
        let _describe_instances_response = match context
            .client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&instance_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!(
                    "Failed to describe instances for '{}': {:?}",
                    instance_name, e
                );
                bail!("Failure describing EC2 instance resources");
            }
        };

        info!(
            "No existing EC2 instance found, creating new EC2 instance '{}'...",
            instance_name
        );

        // Get the network interface id for this node
        let eni_id = self.get_network_interface_id(context, node_index).await?;

        // Build the base run instances request
        let mut run_instances_request = context
            .client
            .run_instances()
            .image_id(&node.image_id)
            .instance_type(aws_sdk_ec2::types::InstanceType::from(
                node.instance_type.as_str(),
            ))
            .min_count(1)
            .max_count(1)
            .key_name(context.ssh_key_name.clone())
            .network_interfaces(
                aws_sdk_ec2::types::InstanceNetworkInterfaceSpecification::builder()
                    .device_index(0)
                    .network_interface_id(&eni_id)
                    .build(),
            );

        // Add placement group if node affinity is enabled and placement group exists
        if context.use_node_affinity {
            if let Some(placement_group_name) = &context.placement_group_name_actual {
                run_instances_request = run_instances_request.placement(
                    aws_sdk_ec2::types::Placement::builder()
                        .group_name(placement_group_name)
                        .build(),
                );
            }
        }

        // Add tags
        run_instances_request = run_instances_request.tag_specifications(
            aws_sdk_ec2::types::TagSpecification::builder()
                .resource_type(aws_sdk_ec2::types::ResourceType::Instance)
                .tags(
                    aws_sdk_ec2::types::Tag::builder()
                        .key("Name")
                        .value(&instance_name)
                        .build(),
                )
                .tags(context.cluster_id_tag.clone())
                .build(),
        );

        let run_instances_response = match run_instances_request.send().await {
            Ok(response) => response,
            Err(e) => {
                error!("Failed to create instance '{}': {:?}", instance_name, e);
                bail!("Failure creating EC2 instance resource");
            }
        };

        if let Some(instance) = run_instances_response.instances().first() {
            if let Some(instance_id) = instance.instance_id() {
                info!(
                    "Requested new instance '{}' with ID '{}'",
                    instance_name, instance_id
                );
                return Ok(instance_id.to_string());
            }
        }

        warn!(
            "Unexpected response when creating instance '{}'",
            instance_name
        );
        bail!("Unexpected response from AWS when creating EC2 instance");
    }

    pub async fn cleanup_ec2_instance(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<()> {
        let instance_name = context.ec2_instance_name(node_index);

        // Find instance by name tag
        let describe_instances_response = match context
            .client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:Name")
                    .values(&instance_name)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!(
                    "Failed to describe instances for '{}': {:?}",
                    instance_name, e
                );
                bail!("Failure describing EC2 instance resources");
            }
        };

        // Check all reservations for instances to terminate and collect their ids
        let mut instance_ids_to_terminate = Vec::new();
        for reservation in describe_instances_response.reservations() {
            for instance in reservation.instances() {
                if let Some(instance_id) = instance.instance_id() {
                    if let Some(state) = instance.state() {
                        if let Some(state_name) = state.name() {
                            match state_name {
                                aws_sdk_ec2::types::InstanceStateName::Running
                                | aws_sdk_ec2::types::InstanceStateName::Pending
                                | aws_sdk_ec2::types::InstanceStateName::Stopped
                                | aws_sdk_ec2::types::InstanceStateName::Stopping => {
                                    info!(
                                        "Found instance to cleanup '{}': '{}' (state: {:?})",
                                        instance_name, instance_id, state_name
                                    );
                                    instance_ids_to_terminate.push(instance_id.to_string());
                                }
                                aws_sdk_ec2::types::InstanceStateName::Terminated
                                | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    info!(
                                        "Instance '{}' is already terminated/terminating",
                                        instance_id
                                    );
                                }
                                _ => {
                                    warn!(
                                        "Instance '{}' is in unexpected state: {:?}",
                                        instance_id, state_name
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Early return if there are no active instances
        if instance_ids_to_terminate.is_empty() {
            info!("No instances found to cleanup for '{}'", instance_name);
            return Ok(());
        }

        // Request instance termination for each collected id
        for instance_id in &instance_ids_to_terminate {
            info!("Terminating instance '{}'...", instance_id);
            match context
                .client
                .terminate_instances()
                .instance_ids(instance_id)
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully initiated termination of instance '{}'",
                        instance_id
                    );
                }
                Err(e) => {
                    error!("Failed to terminate instance '{}': {:?}", instance_id, e);
                    bail!("Failure terminating EC2 instance resource");
                }
            }
        }

        // Wait for all instances to reach a TERMINATED state
        info!("Waiting for instances to fully terminate...");
        let max_wait_time = Duration::from_secs(300); // 5 minutes timeout
        let poll_interval = Duration::from_secs(10); // Poll every 10 seconds
        let start_time = std::time::Instant::now();
        loop {
            if start_time.elapsed() >= max_wait_time {
                warn!(
                    "Timeout waiting for instances to terminate after {} seconds",
                    max_wait_time.as_secs()
                );
                bail!("Timeout waiting for EC2 instances to terminate");
            }

            // Query current state of all instances
            let mut describe_request = context.client.describe_instances();
            for instance_id in &instance_ids_to_terminate {
                describe_request = describe_request.instance_ids(instance_id);
            }
            let describe_response = match describe_request.send().await {
                Ok(response) => response,
                Err(e) => {
                    error!(
                        "Failed to describe instances during termination wait: {:?}",
                        e
                    );
                    bail!("Failure checking instance states during termination");
                }
            };

            // Check state of each instance
            let mut all_terminated = true;
            let mut pending_instances = Vec::new();
            for reservation in describe_response.reservations() {
                for instance in reservation.instances() {
                    if let (Some(instance_id), Some(state)) =
                        (instance.instance_id(), instance.state())
                    {
                        if let Some(state_name) = state.name() {
                            match state_name {
                                aws_sdk_ec2::types::InstanceStateName::Terminated => {
                                    info!(
                                        "Instance '{}' has reached TERMINATED state",
                                        instance_id
                                    );
                                }
                                aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    info!("Instance '{}' is still shutting down...", instance_id);
                                    all_terminated = false;
                                    pending_instances.push(instance_id.to_string());
                                }
                                _ => {
                                    info!(
                                        "Instance '{}' is in state: {:?}",
                                        instance_id, state_name
                                    );
                                    all_terminated = false;
                                    pending_instances.push(instance_id.to_string());
                                }
                            }
                        }
                    }
                }
            }

            if all_terminated {
                info!("All instances have been successfully terminated");
                break;
            }

            info!(
                "Still waiting for {} instance(s) to terminate: {:?}",
                pending_instances.len(),
                pending_instances
            );

            // Wait before next poll
            sleep(poll_interval).await;
        }

        Ok(())
    }

    // Helper method to get ENI ID for a specific node index
    async fn get_network_interface_id(
        &self,
        context: &AwsClusterContext,
        node_index: usize,
    ) -> Result<String> {
        let eni_name = context.network_interface_name(node_index);

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
                bail!("Failure describing network interface for instance creation");
            }
        };

        let network_interfaces = describe_eni_response.network_interfaces();
        if let Some(eni) = network_interfaces.first() {
            if let Some(eni_id) = eni.network_interface_id() {
                return Ok(eni_id.to_string());
            }
        }

        bail!(
            "Network interface '{}' not found for instance creation",
            eni_name
        );
    }
}
