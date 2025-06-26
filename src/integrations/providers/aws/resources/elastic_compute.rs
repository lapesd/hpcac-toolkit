use crate::database::models::Node;
use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn request_elastic_compute_instance_creation(
        &self,
        context: &AwsClusterContext,
        node: &Node,
        node_index: usize,
    ) -> Result<String> {
        let instance_name = context.ec2_instance_name(node_index);

        // Sleep for 20s to give time for the IAM Profile to be propagated
        sleep(Duration::from_secs(20)).await;

        let describe_instances_response = match context
            .ec2_client
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
                error!("{:?}", e);
                bail!("Failure describing EC2 instance resources");
            }
        };

        for reservation in describe_instances_response.reservations() {
            if let Some(instance) = reservation.instances().first() {
                if let Some(instance_id) = instance.instance_id() {
                    if let Some(state) = instance.state() {
                        if let Some(state_name) = state.name() {
                            match state_name {
                                aws_sdk_ec2::types::InstanceStateName::Running
                                | aws_sdk_ec2::types::InstanceStateName::Pending => {
                                    info!(
                                        "Found existing EC2 Instance '{}' in state {:?}, skipping creation",
                                        instance_id, state_name
                                    );
                                    return Ok(instance_id.to_string());
                                }
                                aws_sdk_ec2::types::InstanceStateName::Terminated
                                | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    info!(
                                        "Found existing EC2 Instance '{}' in state {:?}, will create new EC2 Instance",
                                        instance_id, state_name
                                    );
                                    // Continue to create new instance
                                }
                                _ => {
                                    warn!(
                                        "Found existing EC2 Instance '{}' in unexpected state {:?}",
                                        instance_id, state_name
                                    );
                                    bail!(
                                        "Found existing EC2 Instance '{}' in unexpected state '{:?}'. Please check the AWS web panel.",
                                        instance_id,
                                        state_name
                                    )
                                }
                            }
                        }
                    }
                } else {
                    info!(
                        "EC2 Instance '{}' not found, requesting a new one...",
                        instance_name
                    );
                }
            } else {
                info!(
                    "EC2 Instance '{}' not found, requesting a new one...",
                    instance_name
                );
            }
        }

        let eni_id = match context.elastic_network_interface_ids.get(&node_index) {
            Some(id) => id,
            None => {
                warn!(
                    "Elastic Network Interface ids: {:?}",
                    context.elastic_network_interface_ids
                );
                bail!(
                    "Missing expected Elastic Network Interface for Node '{}'",
                    node_index
                );
            }
        };

        let mut run_instances_request = context
            .ec2_client
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
                    .network_interface_id(eni_id)
                    .build(),
            )
            .iam_instance_profile(
                aws_sdk_ec2::types::IamInstanceProfileSpecification::builder()
                    .name(context.iam_profile_name.clone())
                    .build(),
            );

        if node.allocation_mode.to_lowercase() == "spot" {
            run_instances_request = run_instances_request
                .instance_market_options(
                    aws_sdk_ec2::types::InstanceMarketOptionsRequest::builder()
                        .market_type(aws_sdk_ec2::types::MarketType::Spot)
                        .build()
                );
        }

        if let Some(burstable_mode) = &node.burstable_mode {
            let credit_spec = aws_sdk_ec2::types::CreditSpecificationRequest::builder()
                .cpu_credits(burstable_mode.to_lowercase())
                .build();
            run_instances_request = run_instances_request.credit_specification(credit_spec);
        }

        if context.use_node_affinity {
            if let Some(placement_group_name) = &context.placement_group_name_actual {
                run_instances_request = run_instances_request.placement(
                    aws_sdk_ec2::types::Placement::builder()
                        .group_name(placement_group_name)
                        .build(),
                );
            }
        }

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
                error!("{:?}", e);
                bail!("Failure creating EC2 Instance resource");
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

        warn!("{:?}", run_instances_response);
        bail!("Failure finding the id of the requested EC2 Instance");
    }

    pub async fn wait_for_all_elastic_compute_instances_to_be_available(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let instance_ids: Vec<String> = context.ec2_instance_ids.values().cloned().collect();
        if instance_ids.is_empty() {
            info!("No EC2 instances to wait for");
            return Ok(());
        }

        sleep(Duration::from_secs(5)).await;

        let max_wait_time = Duration::from_secs(600);
        let poll_interval = Duration::from_secs(15);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                warn!(
                    "Timeout waiting for EC2 Instances to reach Running state after {} seconds",
                    max_wait_time.as_secs()
                );
                bail!("Timeout waiting for EC2 Instances to reach Running state");
            }

            let mut describe_request = context.ec2_client.describe_instances();
            for instance_id in &instance_ids {
                describe_request = describe_request.instance_ids(instance_id);
            }

            let describe_response = match describe_request.send().await {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure checking EC2 Instance states during status wait");
                }
            };

            let mut all_running = true;
            let mut pending_instances = Vec::new();

            for instance_id in &instance_ids {
                let mut found_instance = false;
                let mut instance_running = false;

                for reservation in describe_response.reservations() {
                    for instance in reservation.instances() {
                        if let Some(current_instance_id) = instance.instance_id() {
                            if current_instance_id == instance_id {
                                found_instance = true;
                                if let Some(state) = instance.state() {
                                    if let Some(state_name) = state.name() {
                                        match state_name {
                                            aws_sdk_ec2::types::InstanceStateName::Running => {
                                                instance_running = true;
                                            }
                                            aws_sdk_ec2::types::InstanceStateName::Pending => {
                                                info!(
                                                    "Instance '{}' is still pending startup...",
                                                    instance_id
                                                );
                                                pending_instances.push(instance_id.clone());
                                            }
                                            aws_sdk_ec2::types::InstanceStateName::Terminated
                                            | aws_sdk_ec2::types::InstanceStateName::ShuttingDown =>
                                            {
                                                error!(
                                                    "Instance '{}' unexpectedly terminated during startup (state: {:?})",
                                                    instance_id, state_name
                                                );
                                                bail!(
                                                    "Instance '{}' terminated unexpectedly during startup",
                                                    instance_id
                                                );
                                            }
                                            _ => {
                                                info!(
                                                    "Instance '{}' is in state: {:?}, waiting for Running",
                                                    instance_id, state_name
                                                );
                                                pending_instances.push(instance_id.clone());
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                }

                if !found_instance {
                    bail!("Instance '{}' not found during status check", instance_id);
                }

                if !instance_running {
                    all_running = false;
                }
            }

            if all_running {
                info!(
                    "All {} instance(s) are now ready and running!",
                    context.ec2_instance_ids.len()
                );
                break;
            }

            if !pending_instances.is_empty() {
                info!(
                    "Still waiting for {} instance(s) to reach Running state: {:?}",
                    pending_instances.len(),
                    pending_instances
                );
            }

            sleep(poll_interval).await;
        }

        Ok(())
    }

    pub async fn request_termination_of_all_elastic_compute_instances(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let describe_instances_response = match context
            .ec2_client
            .describe_instances()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing EC2 Instance resources");
            }
        };

        let mut instances_to_terminate: Vec<String> = Vec::new();
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
                                        "Found cluster instance to terminate: '{}' (state: {:?})",
                                        instance_id, state_name
                                    );
                                    instances_to_terminate.push(instance_id.to_string());
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

        if instances_to_terminate.is_empty() {
            info!(
                "No EC2 instances found for cluster '{}'",
                context.cluster_id
            );
            return Ok(());
        }

        info!(
            "Requesting termination for {} EC2 instance(s) in cluster '{}'...",
            instances_to_terminate.len(),
            context.cluster_id
        );

        match context
            .ec2_client
            .terminate_instances()
            .set_instance_ids(Some(instances_to_terminate.clone()))
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully initiated termination of {} EC2 instance(s): {:?}",
                    instances_to_terminate.len(),
                    instances_to_terminate
                );
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure terminating EC2 Instance resources");
            }
        }

        Ok(())
    }

    pub async fn wait_for_all_elastic_compute_instances_to_be_terminated(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        info!("Ensuring all cluster EC2 instances are terminated...");

        let max_wait_time = Duration::from_secs(900);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                warn!(
                    "Timeout waiting for cluster '{}' EC2 instances to reach Terminated state after {} seconds",
                    context.cluster_id,
                    max_wait_time.as_secs()
                );
                bail!("Timeout waiting for EC2 instances to reach Terminated state");
            }

            let describe_instances_response = match context
                .ec2_client
                .describe_instances()
                .filters(context.cluster_id_filter.clone())
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure describing EC2 instances during termination wait");
                }
            };

            let mut all_terminated = true;
            let mut pending_instances = Vec::new();
            let mut total_instances = 0;

            for reservation in describe_instances_response.reservations() {
                for instance in reservation.instances() {
                    if let Some(instance_id) = instance.instance_id() {
                        total_instances += 1;

                        if let Some(state) = instance.state() {
                            if let Some(state_name) = state.name() {
                                match state_name {
                                    aws_sdk_ec2::types::InstanceStateName::Terminated => {
                                        // Instance is terminated, nothing to do
                                    }
                                    aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                        info!("EC2 instance '{}' is shutting down...", instance_id);
                                        pending_instances.push(instance_id.to_string());
                                        all_terminated = false;
                                    }
                                    aws_sdk_ec2::types::InstanceStateName::Running
                                    | aws_sdk_ec2::types::InstanceStateName::Pending
                                    | aws_sdk_ec2::types::InstanceStateName::Stopped
                                    | aws_sdk_ec2::types::InstanceStateName::Stopping => {
                                        warn!(
                                            "EC2 instance '{}' is still in state: {:?}, expected to be terminating",
                                            instance_id, state_name
                                        );
                                        pending_instances.push(instance_id.to_string());
                                        all_terminated = false;
                                    }
                                    _ => {
                                        info!(
                                            "EC2 instance '{}' is in state: {:?}, waiting for Terminated",
                                            instance_id, state_name
                                        );
                                        pending_instances.push(instance_id.to_string());
                                        all_terminated = false;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if total_instances == 0 {
                info!(
                    "No EC2 instances found for cluster '{}'",
                    context.cluster_id
                );
                break;
            }

            if all_terminated {
                info!(
                    "All {} EC2 instance(s) for cluster '{}' are now terminated!",
                    total_instances, context.cluster_id
                );
                break;
            }

            if !pending_instances.is_empty() {
                info!(
                    "Still waiting for {} instance(s) to reach Terminated state: {:?}",
                    pending_instances.len(),
                    pending_instances
                );
            }

            sleep(poll_interval).await;
        }

        Ok(())
    }
}
