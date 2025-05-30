use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_security_group(&self, context: &AwsClusterContext) -> Result<Vec<String>> {
        let context_vpc_id = context.vpc_id.as_ref().unwrap();

        let describe_security_groups_response = match context
            .client
            .describe_security_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing security group resources");
            }
        };

        let security_groups = describe_security_groups_response.security_groups();
        if !security_groups.is_empty() {
            let mut security_group_ids = Vec::new();
            for sg in security_groups {
                if let Some(sg_id) = sg.group_id() {
                    info!("Found existing security group: '{}'", sg_id);

                    // Verify it's in the correct VPC
                    if let Some(vpc_id) = sg.vpc_id() {
                        if vpc_id == context_vpc_id {
                            security_group_ids.push(sg_id.to_string());
                        } else {
                            warn!(
                                "Security group '{}' is in different VPC '{}', expected '{}'",
                                sg_id, vpc_id, context_vpc_id
                            );
                        }
                    }
                }
            }

            if !security_group_ids.is_empty() {
                info!("Using existing security groups: {:?}", security_group_ids);
                return Ok(security_group_ids);
            }
        }

        info!("No existing security groups found, creating a new one...");

        let create_security_group_response = match context
            .client
            .create_security_group()
            .group_name(context.security_group_name.clone())
            .description("Allow all traffic")
            .vpc_id(context_vpc_id)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::SecurityGroup)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.security_group_name.clone())
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
                bail!("Failure creating security group resource");
            }
        };

        if let Some(security_group_id) = create_security_group_response.group_id() {
            info!("Created new security group '{}'", security_group_id);

            // Add self-referential ingress rule (allow all traffic within the security group)
            info!(
                "Adding self-referential ingress rule to security group '{}'...",
                security_group_id
            );
            match context
                .client
                .authorize_security_group_ingress()
                .group_id(security_group_id)
                .ip_permissions(
                    aws_sdk_ec2::types::IpPermission::builder()
                        .ip_protocol("-1") // All protocols
                        .from_port(0)
                        .to_port(0)
                        .user_id_group_pairs(
                            aws_sdk_ec2::types::UserIdGroupPair::builder()
                                .group_id(security_group_id) // Self-reference
                                .build(),
                        )
                        .build(),
                )
                .send()
                .await
            {
                Ok(_) => {
                    info!(
                        "Successfully added self-referential ingress rule to security group '{}'",
                        security_group_id
                    );
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure adding self-referential ingress rule to security group");
                }
            }

            // Add SSH ingress rule for external access
            info!(
                "Adding SSH ingress rule to security group '{}'...",
                security_group_id
            );
            match context
                .client
                .authorize_security_group_ingress()
                .group_id(security_group_id)
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
                Ok(_) => {
                    info!(
                        "Successfully added SSH ingress rule to security group '{}'",
                        security_group_id
                    );
                    Ok(vec![security_group_id.to_string()])
                }
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure adding SSH ingress rule to security group");
                }
            }
        } else {
            warn!("{:?}", create_security_group_response);
            bail!("Unexpected response from AWS when creating security group resource");
        }
    }

    pub async fn cleanup_security_group(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_security_groups_response = match context
            .client
            .describe_security_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing security group resources");
            }
        };

        let security_groups = describe_security_groups_response.security_groups();
        if security_groups.is_empty() {
            info!("No existing security groups found to cleanup");
            return Ok(());
        }

        for sg in security_groups {
            if let Some(sg_id) = sg.group_id() {
                info!("Found security group to cleanup: '{}'", sg_id);

                // Delete the security group
                info!("Deleting security group '{}'...", sg_id);
                match context
                    .client
                    .delete_security_group()
                    .group_id(sg_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Security group '{}' deleted successfully", sg_id);
                    }
                    Err(e) => {
                        error!("Failed to delete security group '{}': {:?}", sg_id, e);
                        // Continue with other security groups instead of bailing
                        warn!(
                            "Continuing cleanup despite failure to delete security group '{}'",
                            sg_id
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
