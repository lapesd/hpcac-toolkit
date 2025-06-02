use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node};
use crate::integrations::CloudResourceManager;
use crate::utils;

use anyhow::Result;
use tracing::info;

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<()> {
        let mut context = self.create_cluster_context(&cluster)?;
        let resource_count = if cluster.use_node_affinity {
            nodes.len() * 3 + 6 + 1 // Instances + ENIs + EIPs + 6 Shared Resources + Placement Group
        } else {
            nodes.len() * 3 + 6 // Instances + ENIs + EIPs + 6 Shared Resources
        };

        info!(
            "Spawning Cluster '{}' in provider '{}'...",
            cluster.display_name, cluster.provider_id
        );
        info!("Total resources to create: {}", resource_count);

        let multi = utils::ProgressTracker::create_multi();
        let main_progress = utils::ProgressTracker::add_to_multi(
            &multi,
            resource_count as u64,
            Some(&format!(
                "Spawning Cluster: '{}' in '{}'",
                cluster.display_name,
                cluster.provider_id.to_uppercase()
            )),
        );
        let operation_spinner =
            utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

        operation_spinner.update_message("Creating VPC...");
        context.vpc_id = Some(self.ensure_vpc(&context).await?);
        main_progress.inc();

        operation_spinner.update_message("Creating Subnet...");
        context.subnet_id = Some(self.ensure_subnet(&context).await?);
        main_progress.inc();

        operation_spinner.update_message("Creating Internet Gateway...");
        context.gateway_id = Some(self.ensure_internet_gateway(&context).await?);
        main_progress.inc();

        operation_spinner.update_message("Creating Route Table and Routing Rules...");
        // To create a route table and its rules we depend on the VPC, Subnet and Internet
        // Gateway. The ensure_route_table function includes the Route Table creation, the
        // association with the existing Subnet, and the creation of a routing rule through
        // the Internet Gateway
        context.route_table_id = Some(self.ensure_route_table(&context).await?);
        main_progress.inc();

        operation_spinner.update_message("Creating Security Group and Security Rules...");
        // The ensure_security_group function also includes the security rules to allow
        // ingress traffic and SSH between nodes
        context.security_group_ids = self.ensure_security_group(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Importing the SSH key pair...");
        context.ssh_key_id = Some(self.ensure_ssh_key(&context).await?);
        main_progress.inc();

        // Only create a placement group if the node affinity setting is true
        if context.use_node_affinity {
            operation_spinner.update_message("Creating a Placement Group...");
            context.placement_group_name_actual =
                Some(self.ensure_placement_group(&context).await?);
            main_progress.inc();
        }

        for (node_index, _node) in nodes.iter().enumerate() {
            // To create the Elastic Network Interfaces (ENIs), we depend on the previously
            // created Subnet and Security Groups. This method also waits for the ENIs to be
            // attached to a Subnet before returning
            operation_spinner.update_message(&format!(
                "Creating Elastic Network Interface {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.ensure_elastic_network_interface(&context, node_index)
                .await?;
            main_progress.inc();

            // Elastic IPs are allocated instantly, so there's no need to await for them to
            // be created. Instances are ready to associate with the Elastic IP after this
            // method returns
            operation_spinner.update_message(&format!(
                "Creating Elastic IP {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.ensure_elastic_ip(&context, node_index).await?;
            main_progress.inc();
        }

        for (node_index, node) in nodes.iter().enumerate() {
            operation_spinner.update_message(&format!(
                "Creating EC2 Instance {}/{} ({})",
                node_index + 1,
                nodes.len(),
                node.instance_type
            ));
            self.ensure_ec2_instance(&context, node, node_index).await?;
            main_progress.inc();
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
        let resource_count = if cluster.use_node_affinity {
            nodes.len() * 3 + 6 + 1 // Instances + ENIs + EIPs + 6 Shared Resources + Placement Group
        } else {
            nodes.len() * 3 + 6 // Instances + ENIs + EIPs + 6 Shared Resources
        };

        info!(
            "Destroying Cluster '{}' in provider '{}'...",
            cluster.display_name,
            nodes.len()
        );
        info!("Total resources to destroy: {}", resource_count);

        let multi = utils::ProgressTracker::create_multi();
        let main_progress = utils::ProgressTracker::add_to_multi(
            &multi,
            resource_count as u64,
            Some(&format!(
                "Destroying Cluster '{}' in '{}'",
                cluster.display_name,
                cluster.provider_id.to_uppercase()
            )),
        );
        let operation_spinner =
            utils::ProgressTracker::new_indeterminate(&multi, "Initializing...");

        for (node_index, _node) in nodes.iter().enumerate() {
            operation_spinner.update_message(&format!(
                "Destroying EC2 Instance {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.cleanup_ec2_instance(&context, node_index).await?;
            main_progress.inc();
        }

        for (node_index, _node) in nodes.iter().enumerate() {
            operation_spinner.update_message(&format!(
                "Destroying Elastic IP {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.cleanup_elastic_ip(&context, node_index).await?;
            main_progress.inc();

            operation_spinner.update_message(&format!(
                "Destroying Elastic Network Interface {}/{}",
                node_index + 1,
                nodes.len()
            ));
            self.cleanup_elastic_network_interface(&context, node_index)
                .await?;
            main_progress.inc();
        }

        if context.use_node_affinity {
            operation_spinner.update_message("Destroying Placement Group...");
            self.cleanup_placement_group(&context).await?;
            main_progress.inc();
        }

        operation_spinner.update_message("Deregistering the SSH key pair...");
        self.cleanup_ssh_key(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Destroying Security Rules and the Security Group...");
        self.cleanup_security_group(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Destroying Routing Rules and Route Table...");
        self.cleanup_route_table(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Destroying Internet Gateway...");
        self.cleanup_internet_gateway(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Destroying Subnet...");
        self.cleanup_subnet(&context).await?;
        main_progress.inc();

        operation_spinner.update_message("Destroying VPC...");
        self.cleanup_vpc(&context).await?;
        main_progress.inc();

        // Finish all progress bars
        operation_spinner.finish_with_message("All Cloud operations completed");
        main_progress.finish_with_message(&format!(
            "Cluster '{}' destroyed successfully!",
            cluster.display_name
        ));

        println!("Cluster destruction completed successfully!");
        Ok(())
    }
}
