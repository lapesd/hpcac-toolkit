use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node, InstanceCreationFailurePolicy};
use crate::integrations::CloudResourceManager;
use crate::utils;

use std::collections::HashMap;

use anyhow::Result;
use tracing::info;
use tracing::error;
use anyhow::bail;

const MAX_MIGRATION_ATTEMPTS: usize = 3;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(
        &self,
        cluster: Cluster,
        nodes: Vec<Node>,
        init_commands: HashMap<usize, Vec<String>>,
    ) -> Result<()> {

        let attempts: usize = cluster.migration_attempts as usize;
        if attempts >= MAX_MIGRATION_ATTEMPTS {
            bail!("\nMaximum migration of {} attempts reached for cluster '{}'.", attempts, cluster.display_name);
        }

        eprintln!("\n ATTEMPT: {}", attempts+1);

        let mut context = self.create_cluster_context(&cluster)?;
        let mut steps = 8 + (4 * nodes.len()) + nodes.len();
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4 + nodes.len();
        }
        let total_init_commands: usize =
            init_commands.values().map(|commands| commands.len()).sum();
        steps += total_init_commands;

        let spawning_message = format!("Spawning Cluster '{}'...", cluster.display_name);
        info!(spawning_message);
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
         * 16. Wait for EFS mount target to be ready
         * 17. Attach EC2 Instances to EFS mount target
         * 18. Dispatch EC2 Instances initialization commands
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
        for (node_index, _node) in nodes.iter().enumerate() {
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
            context.elastic_ips.insert(node_index, node_public_ip);
            main_progress.inc(1);
        }

        // 14. Request EC2 Instances
        for (node_index, node) in nodes.iter().enumerate() {
            // 14.1. Request EC2 instance creation...
            operation_spinner.update_message(&format!(
                "Requesting {} of {} EC2 Instances (type='{}')",
                node_index + 1,
                nodes.len(),
                node.instance_type
            ));
            let instance_id = match self
                .request_elastic_compute_instance_creation(&context, node, node_index)
                .await {
                    Ok(id) => id,
                    Err(e) => {
                        match cluster.on_instance_creation_failure.as_ref().unwrap() {
                            InstanceCreationFailurePolicy::Cancel => {
                                // Stop the progress bars before printing errors
                                operation_spinner.finish_with_message("Error occurred, cleaning up...");
                                main_progress.finish_with_message("Cluster creation failed.");
                                
                                // Print the error
                                eprintln!("Failed to create instance for node {}: {:#}", node_index+1, e);
                                eprintln!("\nDestroying and canceling the cluster");

                                // Attempt to terminate/cleanup the cluster
                                let cleanup_result = self.destroy_cluster(cluster.clone(), nodes.clone()).await;
                                if let Err(cleanup_err) = cleanup_result {
                                    error!("Failed to cleanup cluster after instance creation failure: {:?}", cleanup_err);
                                }
                            },
                            InstanceCreationFailurePolicy::OnDemand => {
                                // Stop the progress bars before printing errors
                                operation_spinner.finish_with_message("Error occurred, cleaning up...");
                                main_progress.finish_with_message("Cluster creation failed.");
                                
                                // Print the error
                                eprintln!("Failed to create instance for node {} with '{}' allocation: {:#}", node_index+1, nodes[node_index].allocation_mode, e);
                                eprintln!("\nDestroying cluster and changing the allocation mode of node {} to 'on-demand'.", node_index+1);

                                // Attempt to terminate/cleanup the cluster
                                let cleanup_result = self.destroy_cluster(cluster.clone(), nodes.clone()).await;
                                if let Err(cleanup_err) = cleanup_result {
                                    bail!("Failed to cleanup cluster after instance creation failure: {:?}", cleanup_err);
                                }

                                // Update the cluster and the node
                                let mut new_cluster = cluster.clone();
                                new_cluster.migration_attempts += 1;
                                let mut new_nodes = nodes.clone();
                                new_nodes[node_index].allocation_mode = "on-demand".to_string();

                                // Try creating the cluster in the new zone
                                return Box::pin(self.spawn_cluster(
                                    new_cluster,
                                    new_nodes,
                                    init_commands.clone(),
                                )).await;
                            },
                            InstanceCreationFailurePolicy::Migrate => {
                                // Stop the progress bars before printing errors
                                operation_spinner.finish_with_message("Error occurred, cleaning up...");
                                main_progress.finish_with_message("Cluster creation failed.");
                                
                                // Print the error
                                eprintln!("Failed to create instance for node {} in availability zone {}: {:#}", node_index+1, cluster.availability_zone, e);
                                eprintln!("\nDestroying cluster and retrying in a different availability zone.");

                                // Attempt to terminate/cleanup the cluster
                                let cleanup_result = self.destroy_cluster(cluster.clone(), nodes.clone()).await;
                                if let Err(cleanup_err) = cleanup_result {
                                    bail!("Failed to cleanup cluster after instance creation failure: {:?}", cleanup_err);
                                }

                                // Split the tried_zones string into a Vec<&str>, removing empty entries
                                let mut tried_zones: Vec<_> = cluster.tried_zones
                                    .as_deref()
                                    .unwrap_or("")
                                    .split(',')
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                tried_zones.push(cluster.availability_zone.as_str());

                                // Get all available zones in the region
                                let all_zones = self.get_all_availability_zones(&context.ec2_client, &cluster.region).await?;

                                // Filter out the current zone that failed
                                let alternative_zones: Vec<_> = all_zones.into_iter()
                                    .filter(|z| !tried_zones.contains(&z.as_str()))
                                    .collect();
                                if alternative_zones.is_empty() {
                                    bail!("No alternative availability zones available in region {}", cluster.region);
                                }

                                // Try next alternative zone
                                if let Some(zone) = alternative_zones.first() {
                                    eprintln!("Attempting to create cluster in zone {}", zone);

                                    // Update the cluster with the new zone
                                    let mut new_cluster = cluster.clone();
                                    new_cluster.tried_zones = Some(tried_zones.join(","));
                                    new_cluster.availability_zone = zone.clone();
                                    new_cluster.migration_attempts += 1;

                                    // Try creating the cluster in the new zone
                                    return Box::pin(self.spawn_cluster(
                                        new_cluster,
                                        nodes.clone(),
                                        init_commands.clone(),
                                    )).await;
                                }

                                // If get here, all zones failed
                                bail!("Failed to find an availability zone with sufficient capacity");
                            }
                        }
                        return Err(e);
                    }
                };
            context.ec2_instance_ids.insert(node_index, instance_id);
            main_progress.inc(1);
        }

        // 15. Wait for all EC2 Instances to be available
        operation_spinner.update_message("Waiting for all EC2 Instances to be available...");
        self.wait_for_all_elastic_compute_instances_to_be_available(&context)
            .await?;
        main_progress.inc(1);

        if cluster.use_elastic_file_system {
            // 16. Wait for EFS mount target to be ready
            operation_spinner.update_message("Waiting for the EFS mount target to be ready...");
            self.wait_for_elastic_file_system_mount_target_to_be_ready(&context)
                .await?;
            main_progress.inc(1);

            // 17. Attach EC2 Instances to EFS mount target
            for (node_index, _) in nodes.iter().enumerate() {
                let op_msg = format!(
                    "Attaching Node {} of {} to EFS Mount Target...",
                    node_index + 1,
                    nodes.len()
                );
                operation_spinner.update_message(&op_msg);
                let node_instance_id = &context.ec2_instance_ids[&node_index];
                let efs_dns_name = format!(
                    "{}.efs.{}.amazonaws.com",
                    context.efs_device_id.clone().unwrap(),
                    cluster.region,
                );

                // Script to attach instances to EFS
                let efs_attach_script = [
                    "sudo dnf install -y nfs-utils".to_string(),
                    "sudo mkdir -p /shared".to_string(),
                    format!("sudo mount -t nfs4 {}:/ /shared", efs_dns_name),
                    "sudo chmod ugo+rwx /shared".to_string(),
                ];

                for command in efs_attach_script {
                    self.send_and_wait_for_ssm_command(&context, node_instance_id, command)
                        .await?;
                }
                main_progress.inc(1);
            }
        }

        // 18. Dispatch EC2 Instance initialization commands
        for (node_index, _) in nodes.iter().enumerate() {
            let node_instance_id = &context.ec2_instance_ids[&node_index];
            let node_init_commands = &init_commands[&node_index];
            let op_msg = format!(
                "Dispatching {} initialization commands to Instance {} (Node {} of {})...",
                node_init_commands.len(),
                node_instance_id,
                node_index + 1,
                nodes.len()
            );
            operation_spinner.update_message(&op_msg);

            for (cmd_index, command) in node_init_commands.iter().enumerate() {
                let cmd_msg = format!(
                    "Executing command {} of {} on Instance {} (Node {})...",
                    cmd_index + 1,
                    node_init_commands.len(),
                    node_instance_id,
                    node_index + 1
                );
                operation_spinner.update_message(&cmd_msg);

                info!(
                    "Executing command {}/{} on node {}: {}",
                    cmd_index + 1,
                    node_init_commands.len(),
                    node_index + 1,
                    command
                );

                self.send_and_wait_for_ssm_command(&context, node_instance_id, command.clone())
                    .await?;
                main_progress.inc(1);
            }

            info!(
                "Successfully completed all {} initialization commands for Instance {} (Node {})",
                node_init_commands.len(),
                node_instance_id,
                node_index + 1
            );
        }

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

    async fn destroy_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let context = self.create_cluster_context(&cluster)?;
        let mut steps = 9 + (2 * nodes.len());
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += 4;
        }

        let destroying_message = format!("Destroying Cluster '{}'...", cluster.display_name);
        info!(destroying_message);
        let multi = utils::ProgressTracker::create_multi();
        let main_progress =
            utils::ProgressTracker::add_to_multi(&multi, steps as u64, Some(&destroying_message));
        let operation_spinner =
            utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

        /*
         * AWS CLUSTER CLOUD RESOURCE DESTRUCTION CYCLE
         *
         * 1. Request EFS mount target deletion
         * 2. Request termination of all EC2 Instances
         * 3. Wait for EFS mount target to be deleted
         * 4. Request EFS device deletion
         * 5. Wait for all EC2 instances to be terminated
         * 6. for each node {
         *    6.1. Dissociate from ENI device and deallocate Elastic IP
         *    6.2. Destroy ENI device
         * }
         * 7. Destroy Placement Group
         * 8. Destroy SSH Key Pair
         * 9. Destroy Security Groups
         * 10. Destroy IAM Profile
         * 11. Destroy IAM Role and Trust Policies
         * 12. Destroy Route Table
         * 13. Destroy Internet Gateway
         * 14. Destroy Subnet
         * 15. Destroy VPC
         * 16. Wait for EFS device to be deleted
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

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' destroyed successfully!",
            cluster.display_name
        ));
        println!("Cluster destruction completed successfully!");
        Ok(())
    }
}
