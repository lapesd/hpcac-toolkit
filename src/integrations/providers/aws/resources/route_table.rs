use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;

impl AwsInterface {
    pub async fn ensure_route_table(&self, context: &AwsClusterContext) -> Result<String> {
        let context_vpc_id = context.vpc_id.as_ref().unwrap();
        let context_gateway_id = context.gateway_id.as_ref().unwrap();
        let context_subnet_id = context.subnet_id.as_ref().unwrap();

        let describe_route_tables_response = match context
            .ec2_client
            .describe_route_tables()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing Route Table resources: {}", e);
            }
        };

        let route_tables = describe_route_tables_response.route_tables();
        if let Some(route_table) = route_tables.first() {
            if let Some(route_table_id) = route_table.route_table_id() {
                tracing::info!("Found existing Route Table: '{}'", route_table_id);

                if let Some(vpc_id) = route_table.vpc_id() {
                    if vpc_id == context_vpc_id {
                        tracing::info!(
                            "Route Table '{}' is in the correct VPC '{}'",
                            route_table_id,
                            vpc_id
                        );

                        let mut subnet_associated = false;
                        for association in route_table.associations() {
                            if let Some(assoc_subnet_id) = association.subnet_id() {
                                if assoc_subnet_id == context_subnet_id {
                                    subnet_associated = true;
                                    tracing::info!(
                                        "Route Table '{}' already is associated with Subnet '{}'",
                                        route_table_id,
                                        assoc_subnet_id
                                    );
                                    break;
                                }
                            }
                        }

                        if !subnet_associated {
                            tracing::info!(
                                "Associating Route Table '{}' with Subnet '{}'...",
                                route_table_id,
                                context_subnet_id
                            );
                            match context
                                .ec2_client
                                .associate_route_table()
                                .route_table_id(route_table_id)
                                .subnet_id(context_subnet_id)
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    tracing::info!(
                                        "Successfully associated Route Table '{}' with Subnet '{}'",
                                        route_table_id,
                                        context_subnet_id
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("{:?}", e);
                                    anyhow::bail!(
                                        "Failure associating Route Table '{}' with Subnet '{}': {}",
                                        route_table_id,
                                        context_subnet_id,
                                        e
                                    );
                                }
                            }
                        }

                        let mut default_route_exists = false;
                        for route in route_table.routes() {
                            if let Some(dest_cidr) = route.destination_cidr_block() {
                                if dest_cidr == "0.0.0.0/0" {
                                    if let Some(gateway_id) = route.gateway_id() {
                                        if gateway_id == context_gateway_id {
                                            default_route_exists = true;
                                            tracing::info!(
                                                "Default route via Internet Gateway '{}' already exists in Route Table '{}'",
                                                context_gateway_id,
                                                route_table_id
                                            );
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        if !default_route_exists {
                            tracing::info!(
                                "Creating route via Internet Gateway '{}' in Route Table '{}'...",
                                context_gateway_id,
                                route_table_id
                            );
                            match context
                                .ec2_client
                                .create_route()
                                .route_table_id(route_table_id)
                                .destination_cidr_block("0.0.0.0/0")
                                .gateway_id(context_gateway_id)
                                .send()
                                .await
                            {
                                Ok(_) => {
                                    tracing::info!(
                                        "Successfully created route via Internet Gateway '{}' in Route Table '{}'",
                                        context_gateway_id,
                                        route_table_id
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("{:?}", e);
                                    anyhow::bail!(
                                        "Failure creating route via Internet Gateway '{}' in Route Table '{}': {}",
                                        context_gateway_id,
                                        route_table_id,
                                        e
                                    );
                                }
                            }
                        }

                        return Ok(route_table_id.to_string());
                    } else {
                        tracing::error!(
                            "Route Table '{}' is in a different VPC '{}', expected '{}'",
                            route_table_id,
                            vpc_id,
                            context_vpc_id
                        );
                        anyhow::bail!("Route Table is in wrong VPC");
                    }
                }
            }
        }

        tracing::info!("No existing Route Table found, creating a new one...");

        let create_route_table_response = match context
            .ec2_client
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure creating Route Table resource: {}", e);
            }
        };

        if let Some(route_table_id) = create_route_table_response
            .route_table()
            .and_then(|rt| rt.route_table_id())
        {
            tracing::info!("Created new Route Table '{}'", route_table_id);
            tracing::info!(
                "Associating Route Table '{}' with Subnet '{}'...",
                route_table_id,
                context_subnet_id
            );
            match context
                .ec2_client
                .associate_route_table()
                .route_table_id(route_table_id)
                .subnet_id(context_subnet_id)
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!(
                        "Successfully associated Route Table '{}' with Subnet '{}'",
                        route_table_id,
                        context_subnet_id
                    );
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure associating Route Table '{}' with Subnet '{}': {}",
                        route_table_id,
                        context_subnet_id,
                        e
                    );
                }
            }

            tracing::info!(
                "Creating route via Internet Gateway '{}' in Route Table '{}'...",
                context_gateway_id,
                route_table_id,
            );
            match context
                .ec2_client
                .create_route()
                .route_table_id(route_table_id)
                .destination_cidr_block("0.0.0.0/0")
                .gateway_id(context_gateway_id)
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!(
                        "Successfully created route via Internet Gateway '{}' in Route Table '{}'",
                        context_gateway_id,
                        route_table_id
                    );
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure creating route via Internet Gateway '{}' in Route Table '{}': {}",
                        context_gateway_id,
                        route_table_id,
                        e
                    );
                }
            }

            return Ok(route_table_id.to_string());
        }

        tracing::warn!("{:?}", create_route_table_response);
        anyhow::bail!("Failure finding the id of the created Route Table resource");
    }

    pub async fn cleanup_route_table(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_route_tables_response = match context
            .ec2_client
            .describe_route_tables()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing Route Table resources: {}", e);
            }
        };

        let route_tables = describe_route_tables_response.route_tables();
        if let Some(route_table) = route_tables.first() {
            if let Some(route_table_id) = route_table.route_table_id() {
                tracing::info!(
                    "Found existing Route Table to cleanup: '{}'",
                    route_table_id
                );

                for association in route_table.associations() {
                    if association.main().unwrap_or(false) {
                        continue;
                    }

                    if let Some(association_id) = association.route_table_association_id() {
                        tracing::info!("Deleting Route Table association '{}'...", association_id);
                        match context
                            .ec2_client
                            .disassociate_route_table()
                            .association_id(association_id)
                            .send()
                            .await
                        {
                            Ok(_) => {
                                tracing::info!(
                                    "Successfully disassociated Route Table association '{}'",
                                    association_id
                                );
                            }
                            Err(e) => {
                                tracing::error!("{:?}", e);
                                anyhow::bail!("Failure disassociating Route Table: {}", e);
                            }
                        }
                    }
                }

                tracing::info!("Deleting Route Table '{}'...", route_table_id);
                match context
                    .ec2_client
                    .delete_route_table()
                    .route_table_id(route_table_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!("Route Table '{}' deleted successfully", route_table_id);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failure deleting Route Table resource: {}", e);
                    }
                }
            }
        }

        tracing::info!("No existing Route Table found to cleanup");
        Ok(())
    }
}
