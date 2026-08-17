use crate::database::models::Node;
use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;
use tokio::time::{Duration, sleep};

/// Result of a launch. `instance_type` is the type that AWS actually accepted,
/// which differs from the node's preferred type whenever a capacity fallback
/// applied, so callers can persist what really came up.
pub struct InstanceLaunch {
    pub instance_id: String,
    pub instance_type: String,
}

impl AwsInterface {
    /// `candidate_instance_types` is an ordered preference list. The first entry
    /// that AWS has capacity for wins. Pass an empty slice to launch exactly
    /// `node.instance_type` with no fallback.
    pub async fn request_elastic_compute_instance_creation(
        &self,
        context: &AwsClusterContext,
        node: &Node,
        node_index: usize,
        candidate_instance_types: &[String],
    ) -> Result<InstanceLaunch> {
        let instance_name = context.ec2_instance_name(node_index);
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing EC2 instance resources: {}", e);
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
                                    tracing::info!(
                                        "Found existing EC2 Instance '{}' in state {:?}, skipping creation",
                                        instance_id,
                                        state_name
                                    );
                                    // Report the live type, not the node's preferred one: an
                                    // earlier attempt may already have fallen back.
                                    return Ok(InstanceLaunch {
                                        instance_id: instance_id.to_string(),
                                        instance_type: instance
                                            .instance_type()
                                            .map(|t| t.as_str().to_string())
                                            .unwrap_or_else(|| node.instance_type.clone()),
                                    });
                                }
                                aws_sdk_ec2::types::InstanceStateName::Terminated
                                | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    tracing::info!(
                                        "Found existing EC2 Instance '{}' in state {:?}, will create new EC2 Instance",
                                        instance_id,
                                        state_name
                                    );
                                    // Continue to create new instance
                                }
                                _ => {
                                    tracing::warn!(
                                        "Found existing EC2 Instance '{}' in unexpected state {:?}",
                                        instance_id,
                                        state_name
                                    );
                                    anyhow::bail!(
                                        "Found existing EC2 Instance '{}' in unexpected state '{:?}'. Please check the AWS web panel.",
                                        instance_id,
                                        state_name
                                    )
                                }
                            }
                        }
                    }
                } else {
                    tracing::info!(
                        "EC2 Instance '{}' not found, requesting a new one...",
                        instance_name
                    );
                }
            } else {
                tracing::info!(
                    "EC2 Instance '{}' not found, requesting a new one...",
                    instance_name
                );
            }
        }

        let eni_id = match context.elastic_network_interface_ids.get(&node_index) {
            Some(id) => id,
            None => {
                tracing::warn!(
                    "Elastic Network Interface ids: {:?}",
                    context.elastic_network_interface_ids
                );
                anyhow::bail!(
                    "Missing expected Elastic Network Interface for Node '{}'",
                    node_index
                );
            }
        };

        let volume_type = aws_sdk_ec2::types::VolumeType::from(node.root_volume_type.as_str());
        let mut ebs_builder = aws_sdk_ec2::types::EbsBlockDevice::builder()
            .volume_size(node.root_volume_gb as i32)
            .volume_type(volume_type.clone())
            .delete_on_termination(true)
            .encrypted(false);
        if volume_type == aws_sdk_ec2::types::VolumeType::Gp3 {
            ebs_builder = ebs_builder.throughput(500);
        }
        if let Some(iops) = node.root_volume_iops {
            ebs_builder = ebs_builder.iops(iops as i32);
        }
        let block_device_mapping = aws_sdk_ec2::types::BlockDeviceMapping::builder()
            .device_name("/dev/xvda")
            .ebs(ebs_builder.build())
            .build();

        // Instance type is deliberately NOT set here — it is applied per attempt
        // below, so a capacity fallback can vary it without rebuilding everything
        // else (ENI, EBS, tags) that is identical across candidates.
        let mut run_instances_request = context
            .ec2_client
            .run_instances()
            .image_id(&node.image_id)
            .min_count(1)
            .max_count(1)
            .key_name(context.ssh_key_name.clone())
            .network_interfaces(
                aws_sdk_ec2::types::InstanceNetworkInterfaceSpecification::builder()
                    .device_index(0)
                    .network_interface_id(eni_id)
                    .build(),
            )
            .block_device_mappings(block_device_mapping);

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

        // Retry loop just for the run_instances API call. Catches
        // InsufficientInstanceCapacity (transient AWS capacity shortage) and
        // retries only this single request — no need to tear down and rebuild
        // the VPC / subnet / EFS / ENIs / EIPs which are already in place.
        //
        // Each round walks the candidate types in preference order and takes the
        // first that AWS has capacity for. Restarting every round from the head of
        // the list means a preferred type that frees up is still picked over a
        // fallback that was available earlier. Only when every candidate is refused
        // does the round sleep and repeat.
        const MAX_CAPACITY_ROUNDS: u32 = 10;
        const CAPACITY_RETRY_DELAY_SECS: u64 = 30;

        let candidates: Vec<String> = if candidate_instance_types.is_empty() {
            vec![node.instance_type.clone()]
        } else {
            candidate_instance_types.to_vec()
        };

        let mut round: u32 = 0;
        // Kept so the give-up message names what actually blocked the launch,
        // rather than assuming it was a capacity shortage.
        let mut last_transient_error = String::from("none");
        let (run_instances_response, launched_instance_type) = loop {
            let mut accepted = None;
            for candidate in &candidates {
                let attempt = run_instances_request
                    .clone()
                    .instance_type(aws_sdk_ec2::types::InstanceType::from(candidate.as_str()));
                match attempt.send().await {
                    Ok(response) => {
                        accepted = Some((response, candidate.clone()));
                        break;
                    }
                    Err(e) => {
                        let msg = format!("{:?}", e);
                        // Two distinct transient conditions land here.
                        //
                        // InsufficientInstanceCapacity: the provider has no capacity
                        // for this type right now. Another type may still work, so the
                        // loop falls through to the next candidate.
                        //
                        // InvalidNetworkInterface.InUse: on a restore we reuse the
                        // failed node's ENI, and the terminating instance has not
                        // released it yet. AWS itself marks this retryable. Switching
                        // instance type does not help, but waiting does, so this is
                        // purely a matter of retrying until the detach completes.
                        // A fixed pre-launch sleep cannot cover it because detach time
                        // is not bounded.
                        let no_capacity = msg.contains("InsufficientInstanceCapacity");
                        let eni_busy = msg.contains("InvalidNetworkInterface.InUse");
                        if no_capacity || eni_busy {
                            last_transient_error = if eni_busy {
                                "InvalidNetworkInterface.InUse".to_string()
                            } else {
                                format!("InsufficientInstanceCapacity ({})", candidate)
                            };
                            if eni_busy {
                                tracing::warn!(
                                    "Network interface for '{}' is still attached to the terminating instance — will retry",
                                    instance_name
                                );
                            } else if candidates.len() > 1 {
                                tracing::warn!(
                                    "InsufficientInstanceCapacity for '{}' as '{}' — trying next preferred type",
                                    instance_name,
                                    candidate
                                );
                            }
                            continue;
                        }
                        // Anything that is not a capacity shortage (bad AMI for the
                        // type, quota, malformed request) will not improve by
                        // retrying or by switching type, so fail immediately.
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failure creating EC2 Instance resource: {:?}", e);
                    }
                }
            }

            if let Some(accepted) = accepted {
                break accepted;
            }

            round += 1;
            if round > MAX_CAPACITY_ROUNDS {
                anyhow::bail!(
                    "Could not launch '{}' as any of [{}] after {} rounds ({}s). Last transient error: {}",
                    instance_name,
                    candidates.join(", "),
                    MAX_CAPACITY_ROUNDS,
                    MAX_CAPACITY_ROUNDS as u64 * CAPACITY_RETRY_DELAY_SECS,
                    last_transient_error
                );
            }
            tracing::warn!(
                "Could not launch '{}' as any of [{}] — retrying in {}s (round {}/{})",
                instance_name,
                candidates.join(", "),
                CAPACITY_RETRY_DELAY_SECS,
                round,
                MAX_CAPACITY_ROUNDS
            );
            sleep(Duration::from_secs(CAPACITY_RETRY_DELAY_SECS)).await;
        };

        if let Some(instance) = run_instances_response.instances().first() {
            if let Some(instance_id) = instance.instance_id() {
                if launched_instance_type != node.instance_type {
                    tracing::warn!(
                        "Capacity fallback: '{}' launched as '{}' instead of preferred '{}'",
                        instance_name,
                        launched_instance_type,
                        node.instance_type
                    );
                }
                tracing::info!(
                    "Requested new instance '{}' (type='{}') with ID '{}' and {}GB root volume",
                    instance_name,
                    launched_instance_type,
                    instance_id,
                    node.root_volume_gb
                );
                return Ok(InstanceLaunch {
                    instance_id: instance_id.to_string(),
                    instance_type: launched_instance_type,
                });
            }
        }

        tracing::warn!("{:?}", run_instances_response);
        anyhow::bail!("Failure finding the id of the requested EC2 Instance");
    }

    pub async fn wait_for_all_elastic_compute_instances_to_be_available(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let instance_ids: Vec<String> = context.ec2_instance_ids.values().cloned().collect();
        if instance_ids.is_empty() {
            tracing::info!("No EC2 instances to wait for");
            return Ok(());
        }
        let max_wait_time = Duration::from_secs(600);
        let poll_interval = Duration::from_secs(15);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                anyhow::bail!(
                    "Timeout waiting for EC2 instances to pass status checks after {} seconds",
                    max_wait_time.as_secs()
                );
            }

            // describe_instance_status returns both the instance/system status check results.
            // include_all_instances(true) also returns instances still in pending state.
            let mut status_request = context
                .ec2_client
                .describe_instance_status()
                .include_all_instances(true);
            for instance_id in &instance_ids {
                status_request = status_request.instance_ids(instance_id);
            }

            let status_response = match status_request.send().await {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!("Failure checking EC2 instance status checks: {}", e);
                }
            };

            let statuses = status_response.instance_statuses();
            let mut all_ok = true;
            let mut not_ready: Vec<String> = Vec::new();

            for instance_id in &instance_ids {
                let entry = statuses
                    .iter()
                    .find(|s| s.instance_id() == Some(instance_id.as_str()));

                match entry {
                    None => {
                        all_ok = false;
                        not_ready.push(instance_id.clone());
                    }
                    Some(s) => {
                        // Check for unexpected termination first
                        if let Some(state) = s.instance_state() {
                            if let Some(name) = state.name() {
                                match name {
                                    aws_sdk_ec2::types::InstanceStateName::Terminated
                                    | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                        anyhow::bail!(
                                            "Instance '{}' terminated unexpectedly during startup (state: {:?})",
                                            instance_id,
                                            name
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }

                        let instance_ok = s
                            .instance_status()
                            .and_then(|is| is.status())
                            .map(|st| st.as_str() == "ok")
                            .unwrap_or(false);
                        let system_ok = s
                            .system_status()
                            .and_then(|ss| ss.status())
                            .map(|st| st.as_str() == "ok")
                            .unwrap_or(false);

                        if !instance_ok || !system_ok {
                            let inst_st = s
                                .instance_status()
                                .and_then(|is| is.status())
                                .map(|st| st.as_str().to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            let sys_st = s
                                .system_status()
                                .and_then(|ss| ss.status())
                                .map(|st| st.as_str().to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            tracing::info!(
                                "Instance '{}' status checks not yet ok (instance: {}, system: {})",
                                instance_id,
                                inst_st,
                                sys_st
                            );
                            all_ok = false;
                            not_ready.push(instance_id.clone());
                        }
                    }
                }
            }

            if all_ok {
                tracing::info!(
                    "All {} instance(s) passed status checks and are ready",
                    instance_ids.len()
                );
                break;
            }

            tracing::info!(
                "Waiting for {} instance(s) to pass status checks...",
                not_ready.len()
            );
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing EC2 Instance resources: {}", e);
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
                                    tracing::info!(
                                        "Found cluster instance to terminate: '{}' (state: {:?})",
                                        instance_id,
                                        state_name
                                    );
                                    instances_to_terminate.push(instance_id.to_string());
                                }
                                aws_sdk_ec2::types::InstanceStateName::Terminated
                                | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    tracing::info!(
                                        "Instance '{}' is already terminated/terminating",
                                        instance_id
                                    );
                                }
                                _ => {
                                    tracing::warn!(
                                        "Instance '{}' is in unexpected state: {:?}",
                                        instance_id,
                                        state_name
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        if instances_to_terminate.is_empty() {
            tracing::info!(
                "No EC2 instances found for cluster '{}'",
                context.cluster_id
            );
            return Ok(());
        }

        tracing::info!(
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
                tracing::info!(
                    "Successfully initiated termination of {} EC2 instance(s): {:?}",
                    instances_to_terminate.len(),
                    instances_to_terminate
                );
            }
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure terminating EC2 Instance resources: {}", e);
            }
        }

        Ok(())
    }

    pub async fn wait_for_all_elastic_compute_instances_to_be_terminated(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        tracing::info!("Ensuring all cluster EC2 instances are terminated...");

        let max_wait_time = Duration::from_secs(900);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                tracing::warn!(
                    "Timeout waiting for cluster '{}' EC2 instances to reach Terminated state after {} seconds",
                    context.cluster_id,
                    max_wait_time.as_secs()
                );
                anyhow::bail!("Timeout waiting for EC2 instances to reach Terminated state");
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
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure describing EC2 instances during termination wait: {}",
                        e
                    );
                }
            };

            let mut all_terminated = true;
            let mut pending_instances = Vec::new();
            let mut total_instances = 0;

            for reservation in describe_instances_response.reservations() {
                for instance in reservation.instances() {
                    if let Some(instance_id) = instance.instance_id() {
                        if let Some(state) = instance.state() {
                            if let Some(state_name) = state.name() {
                                // Skip instances that were already terminated before
                                // this operation — AWS keeps them visible for ~1 hour.
                                if *state_name == aws_sdk_ec2::types::InstanceStateName::Terminated
                                {
                                    continue;
                                }
                            }
                        }
                        total_instances += 1;

                        if let Some(state) = instance.state() {
                            if let Some(state_name) = state.name() {
                                match state_name {
                                    aws_sdk_ec2::types::InstanceStateName::Terminated => {
                                        // Instance is terminated, nothing to do
                                    }
                                    aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                        tracing::info!(
                                            "EC2 instance '{}' is shutting down...",
                                            instance_id
                                        );
                                        pending_instances.push(instance_id.to_string());
                                        all_terminated = false;
                                    }
                                    aws_sdk_ec2::types::InstanceStateName::Running
                                    | aws_sdk_ec2::types::InstanceStateName::Pending
                                    | aws_sdk_ec2::types::InstanceStateName::Stopped
                                    | aws_sdk_ec2::types::InstanceStateName::Stopping => {
                                        tracing::warn!(
                                            "EC2 instance '{}' is still in state: {:?}, expected to be terminating",
                                            instance_id,
                                            state_name
                                        );
                                        pending_instances.push(instance_id.to_string());
                                        all_terminated = false;
                                    }
                                    _ => {
                                        tracing::info!(
                                            "EC2 instance '{}' is in state: {:?}, waiting for Terminated",
                                            instance_id,
                                            state_name
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
                tracing::info!(
                    "No EC2 instances found for cluster '{}'",
                    context.cluster_id
                );
                break;
            }

            if all_terminated {
                tracing::info!(
                    "All {} EC2 instance(s) for cluster '{}' are now terminated!",
                    total_instances,
                    context.cluster_id
                );
                break;
            }

            if !pending_instances.is_empty() {
                tracing::info!(
                    "Still waiting for {} instance(s) to reach Terminated state: {:?}",
                    pending_instances.len(),
                    pending_instances
                );
            }

            sleep(poll_interval).await;
        }

        Ok(())
    }

    pub async fn find_elastic_compute_instance_by_private_ip(
        &self,
        context: &AwsClusterContext,
        private_ip: &str,
    ) -> Result<Option<String>> {
        tracing::info!("Looking for instance with private IP: {}", private_ip);

        let describe_instances_response = match context
            .ec2_client
            .describe_instances()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("private-ip-address")
                    .values(private_ip)
                    .build(),
            )
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failure describing EC2 instances by private IP '{}': {}",
                    private_ip,
                    e
                );
            }
        };

        for reservation in describe_instances_response.reservations() {
            for instance in reservation.instances() {
                if let Some(instance_id) = instance.instance_id() {
                    if let Some(state) = instance.state() {
                        if let Some(state_name) = state.name() {
                            match state_name {
                                aws_sdk_ec2::types::InstanceStateName::Running
                                | aws_sdk_ec2::types::InstanceStateName::Pending => {
                                    tracing::info!(
                                        "Found running instance '{}' with private IP '{}'",
                                        instance_id,
                                        private_ip
                                    );
                                    return Ok(Some(instance_id.to_string()));
                                }
                                aws_sdk_ec2::types::InstanceStateName::Terminated
                                | aws_sdk_ec2::types::InstanceStateName::ShuttingDown => {
                                    tracing::info!(
                                        "Found instance '{}' with private IP '{}' but it's already terminated/terminating (state: {:?})",
                                        instance_id,
                                        private_ip,
                                        state_name
                                    );
                                    return Ok(None);
                                }
                                _ => {
                                    tracing::warn!(
                                        "Found instance '{}' with private IP '{}' in unexpected state: {:?}",
                                        instance_id,
                                        private_ip,
                                        state_name
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("No instance found with private IP '{}'", private_ip);
        Ok(None)
    }

    pub async fn terminate_elastic_compute_instance(
        &self,
        context: &AwsClusterContext,
        instance_id: &str,
    ) -> Result<()> {
        tracing::info!("Requesting termination of instance '{}'", instance_id);

        match context
            .ec2_client
            .terminate_instances()
            .instance_ids(instance_id)
            .send()
            .await
        {
            Ok(response) => {
                let terminating_instances = response.terminating_instances();
                for terminating_instance in terminating_instances {
                    if let Some(id) = terminating_instance.instance_id() {
                        if let Some(current_state) = terminating_instance.current_state() {
                            if let Some(state_name) = current_state.name() {
                                tracing::info!(
                                    "Instance '{}' termination initiated, current state: {:?}",
                                    id,
                                    state_name
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure terminating instance '{}': {}", instance_id, e);
            }
        }

        Ok(())
    }
}
