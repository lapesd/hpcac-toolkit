use super::interface::AwsInterface;
use crate::database::models::{Cluster, Node};
use crate::integrations::{CloudErrorHandler, CloudResourceManager};

use anyhow::{Error, Result, anyhow};
use std::fs;
use tracing::info;

impl AwsInterface {
    async fn get_existing_vpc(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let vpc_query_response = match client
            .describe_vpcs()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return self.handle_error(e.into(), "Failed to query existing VPCs"),
        };
        let vpcs = vpc_query_response.vpcs();
        if let Some(vpc) = vpcs.first() {
            return Ok(vpc.vpc_id().map(String::from));
        }
        Ok(None)
    }

    async fn create_vpc(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_name: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Creating a new VPC...");
        let vpc_cidr_block = "10.0.0.0/16";
        let vpc_output = match client
            .create_vpc()
            .cidr_block(vpc_cidr_block)
            // TODO: Evaluate the possibility of using Dedicated tenancy
            .instance_tenancy(aws_sdk_ec2::types::Tenancy::Default)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(vpc_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating VPC"),
        };
        let vpc_id = match vpc_output.vpc().and_then(|vpc| vpc.vpc_id()) {
            Some(id) => {
                info!("Created new VPC '{}'", id);
                id
            }
            None => {
                let error_msg = "Missing VPC id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };
        Ok(vpc_id.to_string())
    }

    async fn get_existing_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let subnet_query_response = match client
            .describe_subnets()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => return self.handle_error(e.into(), "Failed to query existing subnets"),
        };

        let subnets = subnet_query_response.subnets();
        if let Some(subnet) = subnets.first() {
            return Ok(subnet.subnet_id().map(String::from));
        }

        Ok(None)
    }

    async fn create_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_id: &str,
        subnet_name: &str,
        availability_zone: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!(
            "Creating a new subnet in availability zone '{}'...",
            availability_zone
        );
        let subnet_cidr_block = "10.0.1.0/24";
        let subnet_output = match client
            .create_subnet()
            .vpc_id(vpc_id)
            .cidr_block(subnet_cidr_block)
            .availability_zone(availability_zone)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Subnet)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(subnet_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating subnet"),
        };

        let subnet_id = match subnet_output.subnet().and_then(|subnet| subnet.subnet_id()) {
            Some(id) => {
                info!("Created new subnet '{}' in AZ '{}'", id, availability_zone);
                id
            }
            None => {
                let error_msg = "Missing Subnet id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(subnet_id.to_string())
    }

    async fn get_existing_internet_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<(String, Option<String>)>> {
        let igw_query_response = match client
            .describe_internet_gateways()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to query existing Internet Gateways");
            }
        };

        let igws = igw_query_response.internet_gateways();
        if let Some(igw) = igws.first() {
            let igw_id = match igw.internet_gateway_id() {
                Some(id) => id.to_string(),
                None => return Ok(None),
            };
            let attachments = igw.attachments();
            let vpc_id = if let Some(attachment) = attachments.first() {
                attachment.vpc_id().map(|id| id.to_string())
            } else {
                None
            };

            return Ok(Some((igw_id, vpc_id)));
        }

        Ok(None)
    }

    async fn create_internet_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        igw_name: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Creating a new Internet Gateway...");
        let igw_output = match client
            .create_internet_gateway()
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::InternetGateway)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(igw_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating Internet Gateway"),
        };

        let igw_id = match igw_output
            .internet_gateway()
            .and_then(|igw| igw.internet_gateway_id())
        {
            Some(id) => {
                info!("Created new Internet Gateway '{}'", id);
                id
            }
            None => {
                let error_msg = "Missing Internet Gateway id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(igw_id.to_string())
    }

    async fn attach_internet_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        igw_id: &str,
        vpc_id: &str,
    ) -> Result<(), Error> {
        info!(
            "Attaching Internet Gateway '{}' to VPC '{}'...",
            igw_id, vpc_id
        );
        match client
            .attach_internet_gateway()
            .internet_gateway_id(igw_id)
            .vpc_id(vpc_id)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully attached Internet Gateway '{}' to VPC '{}'",
                    igw_id, vpc_id
                );
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!(
                    "Failed to attach Internet Gateway '{}' to VPC '{}'",
                    igw_id, vpc_id
                ),
            ),
        }
    }

    async fn get_existing_route_table(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let rt_query_response = match client
            .describe_route_tables()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to query existing Route Tables");
            }
        };

        let route_tables = rt_query_response.route_tables();
        if let Some(rt) = route_tables.first() {
            return Ok(rt.route_table_id().map(String::from));
        }

        Ok(None)
    }

    async fn create_route_table(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_id: &str,
        rt_name: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Creating a new Route Table...");

        let rt_output = match client
            .create_route_table()
            .vpc_id(vpc_id)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::RouteTable)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(rt_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating Route Table"),
        };

        let rt_id = match rt_output.route_table().and_then(|rt| rt.route_table_id()) {
            Some(id) => {
                info!("Created new Route Table '{}'", id);
                id
            }
            None => {
                let error_msg = "Missing Route Table id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(rt_id.to_string())
    }

    async fn get_existing_route_table_subnet_associations(
        &self,
        client: &aws_sdk_ec2::Client,
        route_table_id: &str,
    ) -> Result<Vec<String>> {
        let rt_assoc_query_response = match client
            .describe_route_tables()
            .route_table_ids(route_table_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(
                    e.into(),
                    "Failed to query existing Route Table Associations",
                );
            }
        };

        let mut association_ids = Vec::new();
        let route_tables = rt_assoc_query_response.route_tables();
        for rt in route_tables {
            for assoc in rt.associations() {
                if let Some(assoc_id) = assoc.route_table_association_id() {
                    association_ids.push(assoc_id.to_string());
                }
            }
        }

        Ok(association_ids)
    }

    async fn create_route_table_subnet_association(
        &self,
        client: &aws_sdk_ec2::Client,
        route_table_id: &str,
        subnet_id: &str,
    ) -> Result<String, Error> {
        info!(
            "Creating route table association between Route Table '{}' and Subnet '{}'...",
            route_table_id, subnet_id
        );

        let rt_assoc_output = match client
            .associate_route_table()
            .route_table_id(route_table_id)
            .subnet_id(subnet_id)
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => {
                return self.handle_error(
                    e.into(),
                    &format!(
                        "Failed to create association between Route Table '{}' and Subnet '{}'",
                        route_table_id, subnet_id
                    ),
                );
            }
        };

        let rt_assoc_id = match rt_assoc_output.association_id() {
            Some(id) => {
                info!(
                    "Successfully associated Route Table '{}' with Subnet '{}'",
                    route_table_id, subnet_id
                );
                id
            }
            None => {
                let error_msg = "Missing Route Table Association id from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(rt_assoc_id.to_string())
    }

    async fn create_route_via_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        route_table_id: &str,
        gateway_id: &str,
    ) -> Result<(), Error> {
        let destination_cidr = "0.0.0.0/0".to_string();
        info!(
            "Creating route in Route Table '{}' to '{}' via gateway '{}'...",
            route_table_id, destination_cidr, gateway_id
        );

        match client
            .create_route()
            .route_table_id(route_table_id)
            .destination_cidr_block(&destination_cidr)
            .gateway_id(gateway_id)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully created route to '{}' via gateway '{}' in Route Table '{}'",
                    destination_cidr, gateway_id, route_table_id
                );
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!(
                    "Failed to create route to '{}' via gateway '{}' in Route Table '{}'",
                    destination_cidr, gateway_id, route_table_id
                ),
            ),
        }
    }

    async fn get_existing_security_groups(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Vec<(String, String)>, Error> {
        let sg_query_response = match client
            .describe_security_groups()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to query existing Security Groups");
            }
        };

        let mut security_groups = Vec::new();
        for sg in sg_query_response.security_groups() {
            if let (Some(id), Some(name)) = (sg.group_id(), sg.group_name()) {
                security_groups.push((id.to_string(), name.to_string()));
            }
        }

        Ok(security_groups)
    }

    async fn create_security_groups(
        &self,
        client: &aws_sdk_ec2::Client,
        vpc_id: &str,
        sg_name: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<Vec<String>, Error> {
        let mut security_group_ids = Vec::new();

        // Allow All Security Group
        info!("Creating Allow All security group...");

        let allow_all_sg_output = match client
            .create_security_group()
            .group_name(sg_name)
            .description("Allow all traffic")
            .vpc_id(vpc_id)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::SecurityGroup)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(sg_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to create Allow All security group");
            }
        };

        let allow_all_sg_id = match allow_all_sg_output.group_id() {
            Some(id) => {
                info!("Created Allow All security group with ID '{}'", id);
                id.to_string()
            }
            None => {
                let error_msg = "Missing Security Group ID from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        // Add Allow All ingress rule (self-referential for internal communication)
        match client
            .authorize_security_group_ingress()
            .group_id(&allow_all_sg_id)
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("-1") // All protocols
                    .from_port(0)
                    .to_port(0)
                    .user_id_group_pairs(
                        aws_sdk_ec2::types::UserIdGroupPair::builder()
                            .group_id(&allow_all_sg_id) // Self-reference
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await
        {
            Ok(_) => info!(
                "Added self-referential ingress rule to security group '{}'",
                allow_all_sg_id
            ),
            Err(e) => {
                return self.handle_error(e.into(), "Failed to add self-referential ingress rule");
            }
        }

        // Add ingress rule for external SSH access
        match client
            .authorize_security_group_ingress()
            .group_id(&allow_all_sg_id)
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("tcp")
                    .from_port(22)
                    .to_port(22)
                    .ip_ranges(
                        aws_sdk_ec2::types::IpRange::builder()
                            .cidr_ip("0.0.0.0/0")
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await
        {
            Ok(_) => info!(
                "Added SSH ingress rule to security group '{}'",
                allow_all_sg_id
            ),
            Err(e) => return self.handle_error(e.into(), "Failed to add SSH ingress rule"),
        }
        security_group_ids.push(allow_all_sg_id);

        info!("Successfully created security group");
        Ok(security_group_ids)
    }

    async fn get_existing_placement_group(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let pg_query_response = match client
            .describe_placement_groups()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to query existing Placement Groups");
            }
        };

        let placement_groups = pg_query_response.placement_groups();
        if let Some(pg) = placement_groups.first() {
            return Ok(pg.group_name().map(String::from));
        }

        Ok(None)
    }

    async fn create_placement_group(
        &self,
        client: &aws_sdk_ec2::Client,
        pg_name: &str,
        cluster_id: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Creating a new Placement Group...");

        let _pg_output = match client
            .create_placement_group()
            .group_name(pg_name)
            .strategy(aws_sdk_ec2::types::PlacementStrategy::Cluster)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::PlacementGroup)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(pg_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failure creating Placement Group"),
        };

        // Since the create_placement_group API doesn't return the placement group details,
        // we'll confirm the group was created by querying for it
        let existing_pg = match self.get_existing_placement_group(client, cluster_id).await {
            Ok(Some(name)) => {
                info!("Created new Placement Group '{}'", name);
                name
            }
            _ => {
                let error_msg = "Placement Group creation failed or cannot be verified";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(existing_pg)
    }

    async fn get_existing_ssh_key(
        &self,
        client: &aws_sdk_ec2::Client,
        cluster_id: &str,
    ) -> Result<Option<String>> {
        let key_query_response = match client
            .describe_key_pairs()
            .filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("tag:ClusterId")
                    .values(cluster_id)
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return self.handle_error(e.into(), "Failed to query existing SSH key pairs");
            }
        };

        let key_pairs = key_query_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                info!("Found existing SSH key: '{}'", key_id);
                return Ok(Some(key_id.to_string()));
            }
        }

        Ok(None)
    }

    async fn import_ssh_key(
        &self,
        client: &aws_sdk_ec2::Client,
        key_name: &str,
        public_ssh_key_path: &str,
        cluster_tag: aws_sdk_ec2::types::Tag,
    ) -> Result<String, Error> {
        info!("Importing SSH key pair as '{}'...", key_name);

        let public_key_material = match fs::read_to_string(public_ssh_key_path) {
            Ok(material) => material,
            Err(e) => {
                let error_msg = format!(
                    "Failed to read public key file from '{}'",
                    public_ssh_key_path
                );
                return self.handle_error(e.into(), &error_msg);
            }
        };

        let import_output = match client
            .import_key_pair()
            .key_name(key_name)
            .public_key_material(aws_sdk_ec2::primitives::Blob::new(
                public_key_material.as_bytes(),
            ))
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::KeyPair)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(key_name)
                            .build(),
                    )
                    .tags(cluster_tag)
                    .build(),
            )
            .send()
            .await
        {
            Ok(output) => output,
            Err(e) => return self.handle_error(e.into(), "Failed to import SSH key pair"),
        };

        let key_id = match import_output.key_pair_id() {
            Some(key_id) => {
                info!("Imported SSH key pair '{}'", key_id);
                key_id.to_string()
            }
            None => {
                let error_msg = "Missing SSH key pair name from AWS API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        Ok(key_id)
    }

    async fn delete_ssh_key(
        &self,
        client: &aws_sdk_ec2::Client,
        key_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting SSH key '{}'...", key_id);

        match client.delete_key_pair().key_pair_id(key_id).send().await {
            Ok(_) => {
                info!("SSH key pair '{}' deleted successfully", key_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete SSH key pair '{}'", key_id),
            ),
        }
    }

    async fn delete_placement_group(
        &self,
        client: &aws_sdk_ec2::Client,
        pg_name: &str,
    ) -> Result<(), Error> {
        info!("Deleting Placement Group '{}'...", pg_name);

        match client
            .delete_placement_group()
            .group_name(pg_name)
            .send()
            .await
        {
            Ok(_) => {
                info!("Placement Group '{}' deleted successfully", pg_name);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete Placement Group '{}'", pg_name),
            ),
        }
    }

    async fn delete_security_group(
        &self,
        client: &aws_sdk_ec2::Client,
        sg_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting security group '{}'...", sg_id);
        match client.delete_security_group().group_id(sg_id).send().await {
            Ok(_) => {
                info!("Security group '{}' deleted successfully", sg_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete security group '{}'", sg_id),
            ),
        }
    }

    async fn delete_route_table_subnet_association(
        &self,
        client: &aws_sdk_ec2::Client,
        rt_association_id: &str,
    ) -> Result<(), Error> {
        info!(
            "Deleting Route Table Association '{}'...",
            rt_association_id
        );
        match client
            .disassociate_route_table()
            .association_id(rt_association_id)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Route Table Association '{}' deleted successfully",
                    rt_association_id
                );
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!(
                    "Failed to delete Route Table Association '{}'",
                    rt_association_id
                ),
            ),
        }
    }

    async fn delete_route_table(
        &self,
        client: &aws_sdk_ec2::Client,
        rt_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting Route Table '{}'...", rt_id);
        match client
            .delete_route_table()
            .route_table_id(rt_id)
            .send()
            .await
        {
            Ok(_) => {
                info!("Route Table '{}' deleted successfully", rt_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete Route Table '{}'", rt_id),
            ),
        }
    }

    async fn detach_internet_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        igw_id: &str,
        vpc_id: &str,
    ) -> Result<(), Error> {
        info!(
            "Detaching Internet Gateway '{}' from VPC '{}'...",
            igw_id, vpc_id
        );
        match client
            .detach_internet_gateway()
            .internet_gateway_id(igw_id)
            .vpc_id(vpc_id)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully detached Internet Gateway '{}' from VPC '{}'",
                    igw_id, vpc_id
                );
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!(
                    "Failed to detach Internet Gateway '{}' from VPC '{}'",
                    igw_id, vpc_id
                ),
            ),
        }
    }

    async fn delete_internet_gateway(
        &self,
        client: &aws_sdk_ec2::Client,
        igw_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting Internet Gateway '{}'...", igw_id);
        match client
            .delete_internet_gateway()
            .internet_gateway_id(igw_id)
            .send()
            .await
        {
            Ok(_) => {
                info!("Internet Gateway '{}' deleted successfully", igw_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete Internet Gateway '{}'", igw_id),
            ),
        }
    }

    async fn delete_subnet(
        &self,
        client: &aws_sdk_ec2::Client,
        subnet_id: &str,
    ) -> Result<(), Error> {
        info!("Deleting subnet '{}'...", subnet_id);
        match client.delete_subnet().subnet_id(subnet_id).send().await {
            Ok(_) => {
                info!("Subnet '{}' deleted successfully", subnet_id);
                Ok(())
            }
            Err(e) => self.handle_error(
                e.into(),
                &format!("Failed to delete subnet '{}'", subnet_id),
            ),
        }
    }

    async fn delete_vpc(&self, client: &aws_sdk_ec2::Client, vpc_id: &str) -> Result<(), Error> {
        info!("Deleting VPC '{}'...", vpc_id);
        match client.delete_vpc().vpc_id(vpc_id).send().await {
            Ok(_) => {
                info!("VPC '{}' deleted successfully", vpc_id);
                Ok(())
            }
            Err(e) => self.handle_error(e.into(), &format!("Failed to delete VPC '{}'", vpc_id)),
        }
    }
}

impl CloudResourceManager for AwsInterface {
    async fn spawn_cluster(&self, cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        // Get AWS SDK client
        let client = self.get_ec2_client(&cluster.region)?;
        let cluster_tag = aws_sdk_ec2::types::Tag::builder()
            .key("ClusterId")
            .value(cluster.id.to_string())
            .build();

        // Create VPC
        let vpc_name = format!("{}-VPC", cluster.id);
        let existing_vpc_id = self.get_existing_vpc(&client, &cluster.id).await?;
        let vpc_id = match existing_vpc_id {
            Some(vpc_id) => {
                info!("VPC for this cluster already exists, skipping creation...");
                vpc_id
            }
            None => {
                self.create_vpc(&client, &vpc_name, cluster_tag.clone())
                    .await?
            }
        };

        // Create Subnet
        let subnet_name = format!("{}-SUBNET", cluster.id);
        let existing_subnet_id = self.get_existing_subnet(&client, &cluster.id).await?;
        let subnet_id = match existing_subnet_id {
            Some(subnet_id) => {
                info!("Subnet for this cluster already exists, skipping creation...");
                subnet_id
            }
            None => {
                self.create_subnet(
                    &client,
                    &vpc_id,
                    &subnet_name,
                    &cluster.availability_zone,
                    cluster_tag.clone(),
                )
                .await?
            }
        };

        // Create Internet Gateway and attach it to the VPC
        let igw_name = format!("{}-IGW", cluster.id);
        let existing_igw_id_and_attached_vpc = self
            .get_existing_internet_gateway(&client, &cluster.id)
            .await?;
        let igw_id = match existing_igw_id_and_attached_vpc {
            Some((igw_id, attached_vpc_id)) => {
                info!("Internet gateway for this cluster already exists, skipping creation...");
                if attached_vpc_id.is_none() {
                    self.attach_internet_gateway(&client, &igw_id, &vpc_id)
                        .await?;
                } else {
                    info!("Internet gateway for this cluster is already attached to a VPC.");
                }
                igw_id
            }
            None => {
                let new_igw_id = self
                    .create_internet_gateway(&client, &igw_name, cluster_tag.clone())
                    .await?;
                self.attach_internet_gateway(&client, &new_igw_id, &vpc_id)
                    .await?;
                new_igw_id
            }
        };

        // Create Route Table
        let rt_name = format!("{}-RT", cluster.id);
        let existing_rt_id = self.get_existing_route_table(&client, &cluster.id).await?;
        let rt_id = match existing_rt_id {
            Some(rt_id) => {
                info!("Route Table for this cluster already exists, skipping creation...");
                rt_id
            }
            None => {
                let rt_id = self
                    .create_route_table(&client, &vpc_id, &rt_name, cluster_tag.clone())
                    .await?;
                // TODO: Add get_existing_routes helper method and split route creation
                // from the create route table logic.
                self.create_route_via_gateway(&client, &rt_id, &igw_id)
                    .await?;

                rt_id
            }
        };

        // Create Route Table Subnet Association
        let existing_rt_association_ids = self
            .get_existing_route_table_subnet_associations(&client, &rt_id)
            .await?;
        if existing_rt_association_ids.is_empty() {
            self.create_route_table_subnet_association(&client, &rt_id, &subnet_id)
                .await?;
        } else {
            info!(
                "Route Table subnet association for this cluster already exists, skipping creation..."
            );
        }

        // Create Security Groups
        let existing_sgs = self
            .get_existing_security_groups(&client, &cluster.id)
            .await?;
        if existing_sgs.is_empty() {
            let sg_name = format!("{}-SECURITY_GROUP", &cluster.id);
            self.create_security_groups(&client, &vpc_id, &sg_name, cluster_tag.clone())
                .await?;
        } else {
            info!("Security Groups for this cluster already exists, skipping creation...");
        }

        // Create Placement Group, if cluster is configured to use it
        if cluster.node_affinity {
            let existing_pg = self
                .get_existing_placement_group(&client, &cluster.id)
                .await?;
            if existing_pg.is_none() {
                let pg_name = format!("{}-PG", &cluster.id);
                self.create_placement_group(&client, &pg_name, &cluster.id, cluster_tag.clone())
                    .await?;
            } else {
                info!("Placement Group for this cluster already exists, skipping creation...");
            }
        }

        // Create the SSH Key Pairs
        let existing_key = self.get_existing_ssh_key(&client, &cluster.id).await?;
        if existing_key.is_none() {
            let key_name = format!("{}-SSH-KEY-PAIR", &cluster.id);
            self.import_ssh_key(
                &client,
                &key_name,
                &cluster.public_ssh_key_path,
                cluster_tag.clone(),
            )
            .await?;
        } else {
            info!("SSH Key Pair for this cluster already exists, skipping creation...");
        }

        // TODO: Continue spawning the Cluster
        Ok(())
    }

    /// Function to destroy the cluster
    async fn destroy_cluster(&self, cluster: Cluster) -> Result<(), Error> {
        // Get AWS SDK client
        let client = self.get_ec2_client(&cluster.region)?;

        // Delete SSH KEY
        let existing_key_id = self.get_existing_ssh_key(&client, &cluster.id).await?;
        if existing_key_id.is_some() {
            self.delete_ssh_key(&client, &existing_key_id.unwrap())
                .await?;
        }

        // Delete PLACEMENT GROUP
        if cluster.node_affinity {
            let existing_pg_name = self
                .get_existing_placement_group(&client, &cluster.id)
                .await?;
            if existing_pg_name.is_some() {
                self.delete_placement_group(&client, &existing_pg_name.unwrap())
                    .await?;
            }
        }

        // Delete SECURITY GROUPS
        let existing_sgs = self
            .get_existing_security_groups(&client, &cluster.id)
            .await?;
        for (sg_id, _) in existing_sgs.into_iter() {
            self.delete_security_group(&client, &sg_id).await?;
        }

        // Delete ROUTE TABLE
        let existing_rt_id = self.get_existing_route_table(&client, &cluster.id).await?;
        match existing_rt_id {
            Some(rt_id) => {
                info!("Found route table '{}' for cluster, deleting...", rt_id);
                let existing_rt_association_ids = self
                    .get_existing_route_table_subnet_associations(&client, &rt_id)
                    .await?;
                for id in existing_rt_association_ids {
                    self.delete_route_table_subnet_association(&client, &id)
                        .await?;
                }
                self.delete_route_table(&client, &rt_id).await?;
            }
            None => {
                info!("No route table found for cluster {}", cluster.id);
            }
        }

        // Detach and delete INTERNET GATEWAY
        let existing_igw_id_and_attached_vpc = self
            .get_existing_internet_gateway(&client, &cluster.id)
            .await?;
        match existing_igw_id_and_attached_vpc {
            Some((igw_id, vpc_id)) => {
                info!(
                    "Found internet gateway '{}' for cluster, deleting...",
                    igw_id
                );
                if vpc_id.is_some() {
                    self.detach_internet_gateway(&client, &igw_id, &vpc_id.unwrap())
                        .await?;
                }
                self.delete_internet_gateway(&client, &igw_id).await?;
            }
            None => {
                info!("No internet gateway found for cluster {}", cluster.id);
            }
        }

        // Delete SUBNET
        let existing_subnet_id = self.get_existing_subnet(&client, &cluster.id).await?;
        match existing_subnet_id {
            Some(subnet_id) => {
                info!("Found subnet '{}' for cluster, deleting...", subnet_id);
                self.delete_subnet(&client, &subnet_id).await?;
            }
            None => {
                info!("No subnet found for cluster {}", cluster.id);
            }
        }

        // Delete VPC
        let existing_vpc_id = self.get_existing_vpc(&client, &cluster.id).await?;
        match existing_vpc_id {
            Some(vpc_id) => {
                info!("Found VPC '{}' for cluster, deleting...", vpc_id);
                self.delete_vpc(&client, &vpc_id).await?;
            }
            None => {
                info!("No VPC found for cluster {}", cluster.id);
            }
        }

        Ok(())
    }
}
