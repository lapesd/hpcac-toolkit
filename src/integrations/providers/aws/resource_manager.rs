use super::interface::AwsInterface;

use crate::database::models::{Cluster, ClusterState, Node};
use crate::integrations::CloudResourceManager;
use crate::utils;

use std::collections::HashMap;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tokio::time::{Duration, sleep};
use tracing::info;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<()> {
        let mut context = self.create_cluster_context(&cluster)?;
        let mut steps = 9 + (6 * nodes.len());
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4 + (2 * nodes.len()) + nodes.len();
        } else {
            steps += nodes.len();
        }

        let spawning_message = format!("Spawning Cluster '{}'...", cluster.display_name);
        info!(spawning_message);

        let new_state = match cluster.state {
            ClusterState::Running => ClusterState::Restoring,
            _ => ClusterState::Spawning,
        };
        cluster.update_state(pool, new_state).await?;

        let multi = utils::ProgressTracker::create_multi();
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
         * 7. Create IAM Role and attach Trust Policies
         * 8. Create IAM Profile and assume IAM Role
         * 9. (conditional) Wait for EFS device to be ready
         * 10. (conditional) Request EFS mount target
         * 11. Create SSH Key Pair
         * 12. (conditional) Create Placement Group
         * 13. for each node {
         *     13.1. Create ENI device
         *     13.2. Create Elastic IP
         *     13.3. Associate Elastic IP with ENI device
         * }
         * 14. for each node {
         *     14.1. Request EC2 instance creation
         * }
         * 15. Wait for all EC2 instances to be ready
         * 16. (conditional) Wait for EFS mount target to be ready
         * 17. Wait for SSM agents to be ready on all instances
         * 18. (conditional) Attach EC2 Instances to EFS mount target using SSM
         * 19. (conditional) Dispatch EC2 Instances initialization commands
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

        // 7. Create IAM Role and attach Trust Policies
        operation_spinner.update_message("Creating IAM Role and Trust Policies...");
        self.ensure_iam_role_and_trust_policies(&context).await?;
        main_progress.inc(1);

        // 8. Create IAM Profile and assume IAM Role
        operation_spinner.update_message("Creating IAM Profile and assuming IAM Roles...");
        self.ensure_iam_profile(&context).await?;
        main_progress.inc(1);

        if cluster.use_elastic_file_system {
            // 9. Wait for EFS device to be ready...
            operation_spinner
                .update_message("Waiting for Elastic File System (EFS) device to be ready...");
            self.wait_for_elastic_file_system_device_to_be_ready(&context)
                .await?;
            main_progress.inc(1);

            // 10. Request EFS mount target creation...
            operation_spinner
                .update_message("Requesting Elastic File System (EFS) mount target creation...");
            context.efs_mount_target_id = Some(
                self.request_elastic_file_system_mount_target_creation(&context)
                    .await?,
            );
            main_progress.inc(1);
        }

        // 11. Create SSH Key Pair
        operation_spinner.update_message("Importing the SSH key pair...");
        context.ssh_key_id = Some(self.ensure_ssh_key(&context).await?);
        main_progress.inc(1);

        // 12. Create Placement Group
        if context.use_node_affinity {
            operation_spinner.update_message("Creating a Placement Group...");
            context.placement_group_name_actual =
                Some(self.ensure_placement_group(&context).await?);
            main_progress.inc(1);
        }

        // 13. Create ENI devices, Elastic IPs, and associate them
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
            main_progress.inc(1);

            // 13.2. Create Elastic IP
            operation_spinner.update_message(&format!(
                "Allocating {} of {} Elastic IPs",
                node_index + 1,
                nodes.len()
            ));
            let eip_id = self.ensure_elastic_ip(&context, node_index).await?;
            context.elastic_ip_ids.insert(node_index, eip_id.clone());
            main_progress.inc(1);

            // 13.3. Attach Elastic IP to ENI device
            operation_spinner.update_message(&format!(
                "Associating allocated Elastic IP {} with Elastic Network Interface (ENI) device {}...",
                eip_id, eni_id
            ));
            let node_public_ip = self
                .associate_elastic_ip_with_network_interface(&context, &eip_id, &eni_id)
                .await?;
            context
                .elastic_ips
                .insert(node_index, node_public_ip.clone());
            let node_private_ip = context.network_interface_private_ip(node_index);
            node.set_ips(pool, &node_private_ip, &node_public_ip)
                .await?;
            main_progress.inc(1);
        }

        // 14. Request EC2 Instances
        match cluster.state {
            // Sleep for 20s to give time for the IAM Profile to be propagated the first time the
            // Cluster is created
            ClusterState::Pending | ClusterState::Spawning => {
                operation_spinner
                    .update_message("Giving time for the IAM Profile to be propagated...");
                sleep(Duration::from_secs(20)).await;
            }
            _ => {}
        }
        for (node_index, node) in nodes.iter().enumerate() {
            // 14.1. Request EC2 instance creation...
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

            // 17. Wait for SSM agents to be ready on all instances (for EFS mounting)
            for (node_index, _) in nodes.iter().enumerate() {
                let node_instance_id = &context.ec2_instance_ids[&node_index];
                operation_spinner.update_message(&format!(
                    "Waiting for SSM agent readiness on Node {} of {} (for EFS mounting)...",
                    node_index + 1,
                    nodes.len()
                ));
                self.wait_for_ssm_agent_ready(&context, node_instance_id, Duration::from_secs(300))
                    .await?;
                main_progress.inc(1);
            }

            // 18. Attach EC2 Instances to EFS mount target using SSM
            let efs_dns_name = format!(
                "{}.efs.{}.amazonaws.com",
                context.efs_device_id.clone().unwrap(),
                cluster.region,
            );
            let mut ssm_command_ids: HashMap<usize, String> = HashMap::new();
            for (node_index, _) in nodes.iter().enumerate() {
                let op_msg = format!(
                    "Requesting EFS Mount Target attachment for Node {} of {}...",
                    node_index + 1,
                    nodes.len()
                );
                operation_spinner.update_message(&op_msg);
                match nodes[node_index].was_efs_configured {
                    true => {
                        info!(
                            "Skipping Node {} of {} (already configured for EFS)...",
                            node_index + 1,
                            nodes.len()
                        );
                    }
                    false => {
                        let node_instance_id = &context.ec2_instance_ids[&node_index];
                        let efs_attach_script = format!(
                            r#"
sudo yum install -y nfs-utils
sudo mkdir -p /shared
i=1
while true; do
   echo "EFS mount attempt $i..."
   if sudo mount -t nfs4 {}:/ /shared; then
       echo "EFS mount successful!"
       break
   else
       echo "EFS mount failed, waiting 10 seconds for DNS propagation..."
       sleep 10
       i=$((i + 1))
   fi
done
sudo chown ec2-user:ec2-user /shared
echo "EFS mount and setup complete!"
"#,
                            efs_dns_name
                        );
                        let ssm_command_id = self
                            .create_ssm_command(&context, node_instance_id, efs_attach_script)
                            .await?;
                        ssm_command_ids.insert(node_index, ssm_command_id);
                    }
                }
                main_progress.inc(1);
            }
            for (node_index, _) in nodes.iter().enumerate() {
                let max_wait_time = Duration::from_secs(5 * 60);
                let poll_interval = Duration::from_secs(15);
                let op_msg = format!(
                    "Waiting for Node {} of {} to attach to EFS Mount Target...",
                    node_index + 1,
                    nodes.len()
                );
                operation_spinner.update_message(&op_msg);
                match nodes[node_index].was_efs_configured {
                    true => {}
                    false => {
                        let node_instance_id = &context.ec2_instance_ids[&node_index];
                        self.poll_ssm_command_until_completion(
                            &context,
                            &ssm_command_ids[&node_index],
                            node_instance_id,
                            max_wait_time,
                            poll_interval,
                        )
                        .await?;
                        nodes[node_index]
                            .set_efs_configuration_state(pool, true)
                            .await?;
                    }
                }
                main_progress.inc(1);
            }
        } else {
            // Wait for SSM agents to be ready for init commands (when not using EFS)
            for (node_index, _) in nodes.iter().enumerate() {
                let node_instance_id = &context.ec2_instance_ids[&node_index];
                operation_spinner.update_message(&format!(
                    "Waiting for SSM agent readiness on Node {} of {} (for init commands)...",
                    node_index + 1,
                    nodes.len()
                ));
                self.wait_for_ssm_agent_ready(&context, node_instance_id, Duration::from_secs(300))
                    .await?;
                main_progress.inc(1);
            }
        }

        // 19. Dispatch EC2 Instance initialization commands
        let mut ssm_init_command_ids: HashMap<usize, String> = HashMap::new();
        for (node_index, node) in nodes.iter().enumerate() {
            let node_init_commands = node.get_init_commands(pool).await?;
            let op_msg = format!(
                "Dispatching init script for Node {} of {}...",
                node_index + 1,
                nodes.len()
            );
            operation_spinner.update_message(&op_msg);
            if node_init_commands.is_empty() {
                continue;
            }
            let node_instance_id = &context.ec2_instance_ids[&node_index];
            let node_init_script = node_init_commands.join(" && ");
            let ssm_command_id = self
                .create_ssm_command(&context, node_instance_id, node_init_script)
                .await?;
            ssm_init_command_ids.insert(node_index, ssm_command_id);
            main_progress.inc(1);
        }
        for (node_index, ssm_init_command_id) in ssm_init_command_ids.iter() {
            let node_instance_id = &context.ec2_instance_ids[node_index];
            let max_wait_time = Duration::from_secs(15 * 60);
            let poll_interval = Duration::from_secs(15);
            self.poll_ssm_command_until_completion(
                &context,
                ssm_init_command_id,
                node_instance_id,
                max_wait_time,
                poll_interval,
            )
            .await?;
        }

        cluster.update_state(pool, ClusterState::Running).await?;

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' spawned successfully!",
            cluster.display_name
        ));
        println!("\nCluster spawn completed successfully. You can access your nodes using:");
        for (node_index, ip_address) in context.elastic_ips.iter() {
            println!(
                "Node '10.0.0.{}': ssh ec2-user@{}",
                node_index + 10,
                ip_address
            );
        }
        Ok(())
    }

    async fn terminate_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<()> {
        let context = self.create_cluster_context(&cluster)?;
        let mut steps = 10;
        steps += 2 * nodes.len();
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4;
        }

        let terminating_message = format!("Terminating Cluster '{}'...", cluster.display_name);
        info!(terminating_message);

        cluster
            .update_state(pool, ClusterState::Terminating)
            .await?;
        for node in nodes.iter() {
            node.set_efs_configuration_state(pool, false).await?;
        }

        let multi = utils::ProgressTracker::create_multi();
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
         * 10. Destroy IAM Profile
         * 11. Destroy IAM Role and Trust Policies
         * 12. Destroy Route Table
         * 13. Destroy Internet Gateway
         * 14. Destroy Subnet
         * 15. Destroy VPC
         * 16. (optional) Wait for EFS device to be deleted
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

        // 10. Destroy IAM Profile
        operation_spinner.update_message("Destroying IAM Profile...");
        self.cleanup_iam_profile(&context).await?;
        main_progress.inc(1);

        // 11. Destroy IAM Role
        operation_spinner.update_message("Destroying IAM Role and Trust Policies...");
        self.cleanup_trust_policies_and_iam_role(&context).await?;
        main_progress.inc(1);

        // 12. Destroy Route Table
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

        cluster.update_state(pool, ClusterState::Terminated).await?;

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' terminated successfully!",
            cluster.display_name
        ));
        println!("Cluster termination completed successfully!");
        Ok(())
    }

    async fn simulate_cluster_failure(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<()> {
        let context = self.create_cluster_context(&cluster)?;
        match self
            .find_elastic_compute_instance_by_private_ip(&context, node_private_ip)
            .await?
        {
            Some(id) => {
                println!("Terminating instance with IP: '{}'", node_private_ip);
                self.terminate_elastic_compute_instance(&context, &id)
                    .await?;
            }
            None => {
                println!(
                    "Private IP: '{}' not found in Cluster '{}'",
                    node_private_ip, cluster.display_name
                );
                return Ok(());
            }
        }

        match Node::fetch_by_private_ip(pool, node_private_ip).await? {
            Some(failed_node) => {
                failed_node.set_efs_configuration_state(pool, false).await?;
                println!(
                    "Requested termination for Instance '{}' (failure simulation)",
                    failed_node.id
                );
            }
            None => {
                println!(
                    "Couldn't find Node record (private_ip='{}') in the database",
                    node_private_ip
                );
            }
        }

        Ok(())
    }
}
