use super::interface::AwsInterface;

use crate::database::models::{Cluster, ClusterState, Node};
use crate::integrations::CloudResourceManager;
use crate::utils;
use crate::utils::ssh::SshSession;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tokio::time::{Duration, sleep};

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<()> {
        let mut context = self.create_cluster_context(&cluster).await?;
        let mut steps = 7 + (6 * nodes.len());
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4 + (2 * nodes.len()) + nodes.len();
        } else {
            steps += nodes.len();
        }

        let spawning_message = format!("Spawning Cluster '{}'...", cluster.display_name);
        tracing::info!(spawning_message);

        let is_restore = cluster.state == ClusterState::Running;
        let new_state = if is_restore {
            ClusterState::Restoring
        } else {
            ClusterState::Spawning
        };
        cluster.update_state(pool, new_state).await?;

        let (multi, _guard) = utils::ProgressTracker::create_multi();
        let main_progress =
            utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some(&spawning_message));
        let operation_spinner =
            utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

        /*
         * AWS CLUSTER CLOUD RESOURCE CREATION CYCLE
         *
         * 1. (conditional) Request EFS device
         * 2. Create VPC
         * 3. Create Subnet
         * 4. Create Internet Gateway
         * 5. Create Route Table (and Routing Rules and Internet Gateway and Subnet attachments)
         * 6. Create Security Groups (and attach all of them to the VPC)
         * 7. (conditional) Wait for EFS device to be ready
         * 8. (conditional) Request EFS mount target
         * 9. Create SSH Key Pair
         * 10. (conditional) Create Placement Group
         * 11. for each node {
         *     11.1. Create ENI device
         *     11.2. Create Elastic IP
         *     11.3. Associate Elastic IP with ENI device
         * }
         * 12. for each node {
         *     12.1. Request EC2 instance creation
         * }
         * 13. Wait for all EC2 instances to be ready
         * 14. (conditional) Wait for EFS mount target to be ready
         * 15. Wait for SSH to be ready on all instances
         * 16. (conditional) Attach EC2 Instances to EFS mount target via SSH
         * 17. Dispatch EC2 Instance initialization commands via SSH
         */

        // 1. Request EFS device creation...
        if cluster.use_elastic_file_system {
            operation_spinner
                .update_message("Requesting Elastic File System (EFS) device creation...");
            context.efs_device_id = Some(
                self.request_elastic_file_system_device_creation(&context)
                    .await?,
            );
            main_progress.inc(1);
        }

        // 2. Create VPC
        operation_spinner.update_message("Creating Virtual Private Cloud (VPC)...");
        context.vpc_id = Some(self.ensure_vpc(&context).await?);
        main_progress.inc(1);

        // 3. Create Subnet
        operation_spinner.update_message("Creating Subnet...");
        context.subnet_id = Some(self.ensure_subnet(&context).await?);
        main_progress.inc(1);

        // 4. Create Internet Gateway
        operation_spinner.update_message("Creating Internet Gateway...");
        context.gateway_id = Some(self.ensure_internet_gateway(&context).await?);
        main_progress.inc(1);

        // 5. Create Route Table
        operation_spinner.update_message("Creating Route Table and Routing Rules...");
        context.route_table_id = Some(self.ensure_route_table(&context).await?);
        main_progress.inc(1);

        // 6. Create Security Groups
        operation_spinner.update_message("Creating Security Group and Security Rules...");
        context.security_group_ids = self.ensure_security_group(&context).await?;
        main_progress.inc(1);

        if cluster.use_elastic_file_system {
            // 7. Wait for EFS device to be ready...
            operation_spinner
                .update_message("Waiting for Elastic File System (EFS) device to be ready...");
            self.wait_for_elastic_file_system_device_to_be_ready(&context)
                .await?;
            main_progress.inc(1);

            // 8. Request EFS mount target creation...
            operation_spinner
                .update_message("Requesting Elastic File System (EFS) mount target creation...");
            context.efs_mount_target_id = Some(
                self.request_elastic_file_system_mount_target_creation(&context)
                    .await?,
            );
            main_progress.inc(1);
        }

        // 9. Create SSH Key Pair
        operation_spinner.update_message("Importing the SSH key pair...");
        context.ssh_key_id = Some(self.ensure_ssh_key(&context).await?);
        main_progress.inc(1);

        // 10. Create Placement Group
        if context.use_node_affinity {
            operation_spinner.update_message("Creating a Placement Group...");
            context.placement_group_name_actual =
                Some(self.ensure_placement_group(&context).await?);
            main_progress.inc(1);
        }

        // 13. Create ENI devices; allocate Elastic IP only for node 0 (head node).
        // Worker nodes are reachable via the head node and do not need a public IP.
        for (node_index, node) in nodes.iter().enumerate() {
            // 13.1. Create ENI device
            operation_spinner.update_message(&format!(
                "Creating {} of {} Elastic Network Interface (ENI) devices",
                node_index + 1,
                nodes.len()
            ));
            let eni_id = self
                .ensure_elastic_network_interface(&context, node_index)
                .await?;
            context
                .elastic_network_interface_ids
                .insert(node_index, eni_id.clone());
            let node_private_ip = context.network_interface_private_ip(node_index);
            main_progress.inc(1);

            if node_index == 0 {
                // Head node: allocate and associate an Elastic IP for external SSH access.
                operation_spinner.update_message("Allocating Elastic IP for head node...");
                let eip_id = self.ensure_elastic_ip(&context, node_index).await?;
                context.elastic_ip_ids.insert(node_index, eip_id.clone());
                main_progress.inc(1);

                operation_spinner.update_message(&format!(
                    "Associating Elastic IP {} with head node ENI {}...",
                    eip_id, eni_id
                ));
                let node_public_ip = self
                    .associate_elastic_ip_with_network_interface(&context, &eip_id, &eni_id)
                    .await?;
                context.elastic_ips.insert(node_index, node_public_ip.clone());
                node.set_ips(pool, &node_private_ip, &node_public_ip).await?;
                main_progress.inc(1);
            } else {
                // Worker nodes: private IP only, accessed via head node jump host.
                tracing::info!(
                    "Node {} (private_ip='{}'): no public IP, will be accessed via head node",
                    node_index,
                    node_private_ip
                );
                node.set_ips(pool, &node_private_ip, "").await?;
                main_progress.inc(2); // consume EIP + associate steps
            }
        }

        // 12. Request EC2 Instances
        for (node_index, node) in nodes.iter().enumerate() {
            // 12.1. Request EC2 instance creation...
            // TODO: Add Spot support
            operation_spinner.update_message(&format!(
                "Requesting {} of {} EC2 Instances (type='{}')",
                node_index + 1,
                nodes.len(),
                node.instance_type
            ));
            let instance_id = self
                .request_elastic_compute_instance_creation(&context, node, node_index)
                .await?;
            context.ec2_instance_ids.insert(node_index, instance_id);
            main_progress.inc(1);
        }

        // 15. Wait for all EC2 Instances to be available
        operation_spinner.update_message("Waiting for all EC2 Instances to be available...");
        sleep(Duration::from_secs(5)).await;
        self.wait_for_all_elastic_compute_instances_to_be_available(&context)
            .await?;
        main_progress.inc(1);

        if cluster.use_elastic_file_system {
            // 16. Wait for EFS mount target to be ready
            operation_spinner.update_message("Waiting for the EFS mount target to be ready...");
            self.wait_for_elastic_file_system_mount_target_to_be_ready(&context)
                .await?;
            main_progress.inc(1);
        }

        let private_key_content =
            std::fs::read_to_string(&cluster.private_ssh_key_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read private SSH key '{}': {}",
                    cluster.private_ssh_key_path,
                    e
                )
            })?;

        let head_public_ip = context
            .elastic_ips
            .get(&0)
            .cloned()
            .unwrap_or_default();

        // 17. Wait for SSH to be ready on all instances.
        // Head node is polled directly; workers are reached via the head as a jump host.
        for (node_index, _node) in nodes.iter().enumerate() {
            let ssh = if node_index == 0 {
                operation_spinner.update_message(&format!(
                    "Waiting for SSH on head node ({})...",
                    head_public_ip
                ));
                SshSession::for_aws(&head_public_ip, &cluster.private_ssh_key_path)
            } else {
                let worker_private_ip = context.network_interface_private_ip(node_index);
                operation_spinner.update_message(&format!(
                    "Waiting for SSH on worker node {} ({})...",
                    node_index,
                    worker_private_ip
                ));
                SshSession::for_aws_worker(
                    &worker_private_ip,
                    &head_public_ip,
                    &cluster.private_ssh_key_path,
                )
            };
            ssh.wait_until_ready(Duration::from_secs(300)).await?;
            main_progress.inc(1);
        }

        // 18. (conditional) Attach EC2 Instances to EFS mount target via SSH
        if cluster.use_elastic_file_system {
            let efs_dns_name = format!(
                "{}.efs.{}.amazonaws.com",
                context.efs_device_id.clone().unwrap(),
                cluster.region,
            );
            for (node_index, _node) in nodes.iter().enumerate() {
                operation_spinner.update_message(&format!(
                    "Attaching Node {} of {} to EFS mount target...",
                    node_index + 1,
                    nodes.len()
                ));
                if nodes[node_index].was_efs_configured {
                    tracing::info!(
                        "Skipping Node {} of {} (already configured for EFS)...",
                        node_index + 1,
                        nodes.len()
                    );
                } else {
                    let ssh = if node_index == 0 {
                        SshSession::for_aws(&head_public_ip, &cluster.private_ssh_key_path)
                    } else {
                        let worker_private_ip = context.network_interface_private_ip(node_index);
                        SshSession::for_aws_worker(
                            &worker_private_ip,
                            &head_public_ip,
                            &cluster.private_ssh_key_path,
                        )
                    };
                    let efs_attach_script = format!(
                        r#"
rpm -q nfs-utils &>/dev/null || sudo yum install -y nfs-utils
sudo mkdir -p /shared
MAX_RETRIES=18
i=1
while [ $i -le $MAX_RETRIES ]; do
    echo "EFS mount attempt $i/$MAX_RETRIES..."
    if sudo mount -t nfs4 {efs_dns_name}:/ /shared; then
        echo "EFS mount successful!"
        break
    fi
    if [ $i -eq $MAX_RETRIES ]; then
        echo "EFS mount failed after $MAX_RETRIES attempts, giving up."
        exit 1
    fi
    echo "EFS mount failed, retrying in 10 seconds..."
    sleep 10
    i=$((i + 1))
done
sudo chown ec2-user:ec2-user /shared
echo "EFS mount and setup complete!"
"#
                    );
                    ssh.run_command(&efs_attach_script).await?;
                    nodes[node_index]
                        .set_efs_configuration_state(pool, true)
                        .await?;
                }
                main_progress.inc(1);
            }
        }

        // 19. Dispatch EC2 Instance initialization commands via SSH
        for (node_index, node) in nodes.iter().enumerate() {
            operation_spinner.update_message(&format!(
                "Running base setup on Node {} of {}...",
                node_index + 1,
                nodes.len()
            ));
            let ssh = if node_index == 0 {
                SshSession::for_aws(&head_public_ip, &cluster.private_ssh_key_path)
            } else {
                let worker_private_ip = context.network_interface_private_ip(node_index);
                SshSession::for_aws_worker(
                    &worker_private_ip,
                    &head_public_ip,
                    &cluster.private_ssh_key_path,
                )
            };

            // Install the private key so nodes can SSH to each other (needed for MPI)
            ssh.run_command("mkdir -p ~/.ssh && chmod 700 ~/.ssh")
                .await?;
            ssh.upload_file("$HOME/.ssh/id_rsa", &private_key_content)
                .await?;
            ssh.run_command("chmod 600 ~/.ssh/id_rsa").await?;

            // Ensure tmux is available (required for cluster tasks)
            ssh.run_command(
                "command -v tmux &>/dev/null || { command -v dnf &>/dev/null \
                && sudo dnf install -y tmux || sudo yum install -y tmux; }",
            )
            .await?;

            let node_init_commands = node.get_init_commands(pool).await?;
            if !node_init_commands.is_empty() {
                tracing::info!(
                    "Running {} init command(s) on Node {} of {}...",
                    node_init_commands.len(),
                    node_index + 1,
                    nodes.len()
                );
                let init_script = format!("set -e\n{}", node_init_commands.join("\n"));
                ssh.run_command_streaming(&init_script).await?;
            }
            main_progress.inc(1);
        }

        cluster.update_state(pool, ClusterState::Running).await?;

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' spawned successfully!",
            cluster.display_name
        ));
        tracing::info!(
            "Cluster spawn completed successfully. Head node: ssh ec2-user@{}",
            head_public_ip
        );

        if !is_restore && nodes.iter().any(|n| n.allocation_mode == "spot") {
            if let Err(e) = self
                .ensure_spot_interruption_queue(&cluster.id, &cluster.region)
                .await
            {
                tracing::warn!("Could not set up spot interruption queue: {}", e);
            }
        }

        Ok(())
    }

    async fn terminate_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<()> {
        let context = self.create_cluster_context(&cluster).await?;
        let mut steps = 8;
        steps += 2 * nodes.len();
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4;
        }

        let terminating_message = format!("Terminating Cluster '{}'...", cluster.display_name);
        tracing::info!(terminating_message);

        let has_spot_nodes = nodes.iter().any(|n| n.allocation_mode == "spot");
        if has_spot_nodes {
            if let Ok(Some(queue_url)) = self
                .get_spot_interruption_queue_url(&cluster.id, &cluster.region)
                .await
            {
                if let Err(e) = self
                    .cleanup_spot_interruption_queue(&cluster.id, &cluster.region, &queue_url)
                    .await
                {
                    tracing::warn!("Could not clean up spot interruption queue: {}", e);
                }
            }
        }

        cluster
            .update_state(pool, ClusterState::Terminating)
            .await?;
        for node in nodes.iter() {
            node.set_efs_configuration_state(pool, false).await?;
        }

        let (multi, _guard) = utils::ProgressTracker::create_multi();
        let main_progress =
            utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some(&terminating_message));
        let operation_spinner =
            utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

        /*
         * AWS CLUSTER CLOUD RESOURCE DESTRUCTION CYCLE
         *
         * 1. (optional) Request EFS mount target deletion
         * 2. Request termination of all EC2 Instances
         * 3. (optional) Wait for EFS mount target to be deleted
         * 4. (optional) Request EFS device deletion
         * 5. Wait for all EC2 instances to be terminated
         * 6. for each node {
         *    6.1. Dissociate from ENI device and deallocate Elastic IP
         *    6.2. Destroy ENI device
         * }
         * 7. (optional) Destroy Placement Group
         * 8. Destroy SSH Key Pair
         * 9. Destroy Security Groups
         * 10. Destroy Route Table
         * 11. Destroy Internet Gateway
         * 12. Destroy Subnet
         * 13. Destroy VPC
         * 14. (optional) Wait for EFS device to be deleted
         */

        // 1. Request EFS mount target deletion
        if cluster.use_elastic_file_system {
            operation_spinner
                .update_message("Requesting Elastic File System (EFS) mount target deletion...");
            self.request_elastic_file_system_mount_target_deletion(&context)
                .await?;
            main_progress.inc(1);
        }

        // 2. Request termination of all EC2 Instances
        operation_spinner.update_message(&format!(
            "Requesting termination of {} Elastic Compute Instances...",
            nodes.len()
        ));
        self.request_termination_of_all_elastic_compute_instances(&context)
            .await?;
        main_progress.inc(nodes.len() as u64);

        if cluster.use_elastic_file_system {
            // 3. Wait for EFS mount target to be deleted
            operation_spinner
                .update_message("Waiting for Elastic File System (EFS) mount target deletion...");
            self.wait_for_elastic_file_system_mount_target_to_be_deleted(&context)
                .await?;
            main_progress.inc(1);

            // 4. Request EFS device deletion
            operation_spinner
                .update_message("Requesting Elastic File System (EFS) device deletion...");
            self.request_elastic_file_system_device_deletion(&context)
                .await?;
            main_progress.inc(1);
        }

        // 5. Wait for all instances to be terminated
        operation_spinner.update_message(&format!(
            "Waiting for {} Elastic Compute Instances to be terminated...",
            nodes.len()
        ));
        self.wait_for_all_elastic_compute_instances_to_be_terminated(&context)
            .await?;
        main_progress.inc(nodes.len() as u64);

        for (node_index, _node) in nodes.iter().enumerate() {
            // 6.1. Dissociate from ENI device and deallocate Elastic IP
            operation_spinner.update_message(&format!(
                "Destroying Elastic IP {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.cleanup_elastic_ip(&context, node_index).await?;
            main_progress.inc(1);

            // 6.2. Destroy ENI device
            operation_spinner.update_message(&format!(
                "Destroying Elastic Network Interface {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.cleanup_elastic_network_interface(&context, node_index)
                .await?;
            main_progress.inc(1);
        }

        // 7. Destroy Placement Group
        if context.use_node_affinity {
            operation_spinner.update_message("Destroying Placement Group...");
            self.cleanup_placement_group(&context).await?;
            main_progress.inc(1);
        }

        // 8. Destroy SSH Key Pair
        operation_spinner.update_message("Deregistering the SSH key pair...");
        self.cleanup_ssh_key(&context).await?;
        main_progress.inc(1);

        // 9. Destroy Security Groups
        operation_spinner.update_message("Destroying Security Rules and the Security Group...");
        self.cleanup_security_group(&context).await?;
        main_progress.inc(1);

        // 10. Destroy Route Table
        operation_spinner.update_message("Destroying Routing Rules and Route Table...");
        self.cleanup_route_table(&context).await?;
        main_progress.inc(1);

        // 13. Destroy Internet Gateway
        operation_spinner.update_message("Destroying Internet Gateway...");
        self.cleanup_internet_gateway(&context).await?;
        main_progress.inc(1);

        // 14. Destroy Subnet
        operation_spinner.update_message("Destroying Subnet...");
        self.cleanup_subnet(&context).await?;
        main_progress.inc(1);

        // 15. Destroy VPC
        operation_spinner.update_message("Destroying VPC...");
        self.cleanup_vpc(&context).await?;
        main_progress.inc(1);

        // 16. Wait for EFS device to be deleted
        if cluster.use_elastic_file_system {
            operation_spinner.update_message("Waiting for EFS device to be deleted...");
            self.wait_for_elastic_file_system_device_to_be_deleted(&context)
                .await?;
            main_progress.inc(1);
        }

        for node in &nodes {
            node.reset(pool).await?;
        }
        cluster.update_state(pool, ClusterState::Pending).await?;

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' terminated successfully!",
            cluster.display_name
        ));
        tracing::info!(
            "Cluster '{}' is ready to spawn again.",
            cluster.display_name
        );
        Ok(())
    }

    async fn simulate_cluster_failure(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<()> {
        let context = self.create_cluster_context(&cluster).await?;
        match self
            .find_elastic_compute_instance_by_private_ip(&context, node_private_ip)
            .await?
        {
            Some(id) => {
                tracing::info!("Terminating instance with IP: '{}'", node_private_ip);
                self.terminate_elastic_compute_instance(&context, &id)
                    .await?;
            }
            None => {
                tracing::warn!(
                    "Private IP: '{}' not found in Cluster '{}'",
                    node_private_ip,
                    cluster.display_name
                );
                return Ok(());
            }
        }

        match Node::fetch_by_private_ip(pool, node_private_ip).await? {
            Some(failed_node) => {
                failed_node.set_efs_configuration_state(pool, false).await?;
                tracing::info!(
                    "Requested termination for Instance '{}' (failure simulation)",
                    failed_node.id
                );
            }
            None => {
                tracing::warn!(
                    "Couldn't find Node record (private_ip='{}') in the database",
                    node_private_ip
                );
            }
        }

        Ok(())
    }

    async fn check_cluster_health(
        &self,
        _pool: &SqlitePool,
        cluster: &Cluster,
    ) -> Result<Vec<String>> {
        let context = self.create_cluster_context(cluster).await?;

        let response = match context
            .ec2_client
            .describe_instances()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => anyhow::bail!("Failed to describe instances for health check: {}", e),
        };

        let mut failed_ips = Vec::new();
        for reservation in response.reservations() {
            for instance in reservation.instances() {
                let state = instance.state().and_then(|s| s.name());
                if let Some(
                    aws_sdk_ec2::types::InstanceStateName::Terminated
                    | aws_sdk_ec2::types::InstanceStateName::ShuttingDown
                    | aws_sdk_ec2::types::InstanceStateName::Stopped
                    | aws_sdk_ec2::types::InstanceStateName::Stopping,
                ) = state
                    && let Some(ip) = instance.private_ip_address()
                {
                    tracing::info!(
                        "Instance (private_ip='{}') is in failed state '{:?}'",
                        ip,
                        state
                    );
                    failed_ips.push(ip.to_string());
                }
            }
        }

        Ok(failed_ips)
    }
}
