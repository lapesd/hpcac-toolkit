use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_placement_group(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_placement_groups_response = match context
            .client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing placement group resources");
            }
        };

        let placement_groups = describe_placement_groups_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                info!("Found existing placement group: '{}'", group_name);
                return Ok(group_name.to_string());
            }
        }

        info!("No existing placement group found, creating a new one...");

        let create_placement_group_response = match context
            .client
            .create_placement_group()
            .group_name(context.placement_group_name.clone())
            .strategy(aws_sdk_ec2::types::PlacementStrategy::Cluster)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::PlacementGroup)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.placement_group_name.clone())
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
                bail!("Failure creating placement group resource");
            }
        };

        // Since create_placement_group doesn't return placement group details,
        // we need to verify it was created by querying for it
        info!("Verifying placement group creation...");
        let verify_response = match context
            .client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure verifying placement group creation");
            }
        };

        let placement_groups = verify_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                info!("Successfully created placement group '{}'", group_name);
                return Ok(group_name.to_string());
            }
        }

        warn!("{:?}", create_placement_group_response);
        bail!("Placement group creation could not be verified");
    }

    pub async fn cleanup_placement_group(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_placement_groups_response = match context
            .client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing placement group resources");
            }
        };

        let placement_groups = describe_placement_groups_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                info!("Found placement group to cleanup: '{}'", group_name);
                info!("Deleting placement group '{}'...", group_name);
                match context
                    .client
                    .delete_placement_group()
                    .group_name(group_name)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("Placement group '{}' deleted successfully", group_name);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Failed to delete placement group '{}': {:?}", group_name, e);
                        bail!("Failure deleting placement group resource");
                    }
                }
            }
        }

        info!("No existing placement group found to cleanup");
        Ok(())
    }
}
