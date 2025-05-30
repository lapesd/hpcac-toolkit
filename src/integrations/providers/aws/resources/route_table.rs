use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_route_table(&self, context: &AwsClusterContext) -> Result<String> {
        let context_vpc_id = context.vpc_id.as_ref().unwrap();
        let context_gateway_id = context.gateway_id.as_ref().unwrap();
        let context_subnet_id = context.subnet_id.as_ref().unwrap();

        let describe_route_tables_response = match context
            .client
            .describe_route_tables()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing route table resources");
            }
        };

        let route_tables = describe_route_tables_response.route_tables();
        if let Some(route_table) = route_tables.first() {
            if let Some(route_table_id) = route_table.route_table_id() {
                info!("Found existing route table: '{}'", route_table_id);

                // Check if it's in the correct VPC
                if let Some(vpc_id) = route_table.vpc_id() {
                    if vpc_id == context_vpc_id {
                        info!(
                            "Route table '{}' is in the correct VPC '{}'",
                            route_table_id, vpc_id
                        );

                        // Check if subnet association exists
                        let mut subnet_associated = false;
                        for association in route_table.associations() {
                            if let Some(assoc_subnet_id) = association.subnet_id() {
                                if assoc_subnet_id == context_subnet_id {
                                    subnet_associated = true;
                                    break;
                                }
                            }
                        }

                        // Create subnet association if it doesn't exist
                        if !subnet_associated {
                            info!(
                                "Associating route table '{}' with subnet '{}'...",
                                route_table_id, context_subnet_id
                            );
                            match context
                                .client
                                .associate_route_table()
                                .route_table_id(route_table_id)
                                .subnet_id(context_subnet_id)
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Successfully associated route table '{}' with subnet '{}'",
                                        route_table_id, context_subnet_id
                                    );
                                }
                                Err(e) => {
                                    error!("{:?}", e);
                                    bail!("Failure associating route table with subnet");
                                }
                            }
                        }

                        // Check if default route to IGW exists
                        let mut default_route_exists = false;
                        for route in route_table.routes() {
                            if let Some(dest_cidr) = route.destination_cidr_block() {
                                if dest_cidr == "0.0.0.0/0" {
                                    if let Some(gateway_id) = route.gateway_id() {
                                        if gateway_id == context_gateway_id {
                                            default_route_exists = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        // Create default route if it doesn't exist
                        if !default_route_exists {
                            info!(
                                "Creating default route in route table '{}' via gateway '{}'...",
                                route_table_id, context_gateway_id
                            );
                            match context
                                .client
                                .create_route()
                                .route_table_id(route_table_id)
                                .destination_cidr_block("0.0.0.0/0")
                                .gateway_id(context_gateway_id)
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Successfully created default route via gateway '{}' in route table '{}'",
                                        context_gateway_id, route_table_id
                                    );
                                }
                                Err(e) => {
                                    error!("{:?}", e);
                                    bail!("Failure creating default route in route table");
                                }
                            }
                        }

                        return Ok(route_table_id.to_string());
                    } else {
                        error!(
                            "Route table '{}' is in a different VPC '{}', expected '{}'",
                            route_table_id, vpc_id, context_vpc_id
                        );
                        bail!("Route table is in wrong VPC");
                    }
                }
            }
        }

        info!("No existing route table found, creating a new one...");

        let create_route_table_response = match context
            .client
            .create_route_table()
            .vpc_id(context_vpc_id)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::RouteTable)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.route_table_name.clone())
                            .build(),
                    )
                    .tags(context.cluster_id_tag.clone())
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure creating route table resource");
            }
        };

        if let Some(route_table_id) = create_route_table_response
            .route_table()
            .and_then(|rt| rt.route_table_id())
        {
            info!("Created new route table '{}'", route_table_id);

            // Associate with subnet
            info!(
                "Associating route table '{}' with subnet '{}'...",
                route_table_id, context_subnet_id
            );
            match context
                .client
                .associate_route_table()
                .route_table_id(route_table_id)
                .subnet_id(context_subnet_id)
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully associated route table '{}' with subnet '{}'",
                        route_table_id, context_subnet_id
                    );
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure associating route table with subnet");
                }
            }

            // Create default route to IGW
            info!(
                "Creating default route in route table '{}' via gateway '{}'...",
                route_table_id, context_gateway_id
            );
            match context
                .client
                .create_route()
                .route_table_id(route_table_id)
                .destination_cidr_block("0.0.0.0/0")
                .gateway_id(context_gateway_id)
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully created default route via gateway '{}' in route table '{}'",
                        context_gateway_id, route_table_id
                    );
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure creating default route in route table");
                }
            }

            return Ok(route_table_id.to_string());
        }

        warn!("{:?}", create_route_table_response);
        bail!("Unexpected response from AWS when creating route table resource");
    }

    pub async fn cleanup_route_table(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_route_tables_response = match context
            .client
            .describe_route_tables()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing route table resources");
            }
        };

        let route_tables = describe_route_tables_response.route_tables();
        if let Some(route_table) = route_tables.first() {
            if let Some(route_table_id) = route_table.route_table_id() {
                info!(
                    "Found existing route table to cleanup: '{}'",
                    route_table_id
                );

                // First, disassociate from subnets
                for association in route_table.associations() {
                    // Skip main route table associations (they can't be deleted)
                    if association.main().unwrap_or(false) {
                        continue;
                    }

                    if let Some(association_id) = association.route_table_association_id() {
                        info!(
                            "Disassociating route table association '{}'...",
                            association_id
                        );
                        match context
                            .client
                            .disassociate_route_table()
                            .association_id(association_id)
                            .send()
                            .await
                        {
                            Ok(_) => {
                                info!(
                                    "Successfully disassociated route table association '{}'",
                                    association_id
                                );
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                bail!("Failure disassociating route table");
                            }
                        }
                    }
                }

                info!("Deleting route table '{}'...", route_table_id);
                match context
                    .client
                    .delete_route_table()
                    .route_table_id(route_table_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Route table '{}' deleted successfully", route_table_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Failure deleting route table resource");
                    }
                }
            }
        }

        info!("No existing route table found to cleanup");
        Ok(())
    }
}
