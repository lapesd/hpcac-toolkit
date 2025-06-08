use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;
use crate::utils;

use anyhow::Result;
use tracing::info;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let mut context = self.create_cluster_context(&cluster)?;
        let mut steps = (4 * nodes.len()) + 6;
        if cluster.use_node_affinity {
            steps += 1;
        }
        if cluster.use_elastic_file_system {
            steps += nodes.len() + 4;
        }

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
         * 14. Wait for EFS mount target to be ready
         * 15. Attach EC2 Instances to EFS mount target
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

        // 11. Create ENI devices, Elastic IPs, and associate them
        for (node_index, _node) in nodes.iter().enumerate() {
            // 11.1. Create ENI device
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

            // 11.2. Create Elastic IP
            operation_spinner.update_message(&format!(
                "Allocating {} of {} Elastic IPs",
                node_index + 1,
                nodes.len()
            ));
            let eip_id = self.ensure_elastic_ip(&context, node_index).await?;
            context.elastic_ip_ids.insert(node_index, eip_id.clone());
            main_progress.inc(1);

            // 11.3. Attach Elastic IP to ENI device
            operation_spinner.update_message(&format!(
                "Associating allocated Elastic IP {} with Elastic Network Interface (ENI) device {}...",
                eip_id, eni_id
            ));
            self.associate_elastic_ip_with_network_interface(&context, &eip_id, &eni_id)
                .await?;
            main_progress.inc(1);
        }

        // 12. Request EC2 Instances
        // TODO: Add Spot support
        for (node_index, node) in nodes.iter().enumerate() {
            // 12.1. Request EC2 instance creation...
            operation_spinner.update_message(&format!(
                "Requesting {} of {} EC2 Instances (type='{}')",
                node_index + 1,
                nodes.len(),
                node.instance_type
            ));
            let instance_id = self
                .request_elastic_compute_instance_creation(&context, node, node_index)
                .await?;
            context.ec2_instance_ids.push(instance_id);
            main_progress.inc(1);
        }

        // 13. Wait for all EC2 Instances to be available
        operation_spinner.update_message("Waiting for all EC2 Instances to be available...");
        self.wait_for_all_elastic_compute_instances_to_be_available(&context)
            .await?;
        main_progress.inc(1);

        if cluster.use_elastic_file_system {
            // 14. Wait for EFS mount target to be ready
            operation_spinner.update_message("Waiting for the EFS mount target to be ready...");
            self.wait_for_elastic_file_system_mount_target_to_be_ready(&context)
                .await?;
            main_progress.inc(1);

            // 15. Attach EC2 Instances to EFS mount target
            for (node_index, _) in nodes.iter().enumerate() {
                let op_msg = format!(
                    "Attaching Node {} of {} to EFS Mount Target...",
                    node_index,
                    nodes.len()
                );
                operation_spinner.update_message(&op_msg);
                // TODO: attach EC2 instances to EFS mount target (this is done via efs mount helper)
                main_progress.inc(1);
            }
        }

        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' spawned successfully!",
            cluster.display_name
        ));
        println!("Cluster spawn completed successfully");
        Ok(())
    }

    async fn destroy_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let context = self.create_cluster_context(&cluster)?;
        let mut steps = (4 * nodes.len()) + 6;
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
         * 10. Destroy Route Table
         * 11. Destroy Internet Gateway
         * 12. Destroy Subnet
         * 13. Destroy VPC
         * 14. Wait for EFS device to be deleted
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

        // 11. Destroy Internet Gateway
        operation_spinner.update_message("Destroying Internet Gateway...");
        self.cleanup_internet_gateway(&context).await?;
        main_progress.inc(1);

        // 12. Destroy Subnet
        operation_spinner.update_message("Destroying Subnet...");
        self.cleanup_subnet(&context).await?;
        main_progress.inc(1);

        // 13. Destroy VPC
        operation_spinner.update_message("Destroying VPC...");
        self.cleanup_vpc(&context).await?;
        main_progress.inc(1);

        // 14. Wait for EFS device to be deleted
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
