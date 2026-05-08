use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;

impl AwsInterface {
    pub async fn ensure_placement_group(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_placement_groups_response = match context
            .ec2_client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing Placement Group resources: {}", e);
            }
        };

        let placement_groups = describe_placement_groups_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                tracing::info!("Found existing Placement Group: '{}'", group_name);
                return Ok(group_name.to_string());
            }
        }

        tracing::info!("No existing Placement Group found, creating a new one...");

        let create_placement_group_response = match context
            .ec2_client
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure creating Placement Group resource: {}", e);
            }
        };

        // Since create_placement_group doesn't return placement group details,
        // we need to verify it was created by querying for it
        tracing::info!("Verifying Placement Group creation...");
        let verify_response = match context
            .ec2_client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure verifying Placement Group creation: {}", e);
            }
        };

        let placement_groups = verify_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                tracing::info!("Successfully created Placement Group '{}'", group_name);
                return Ok(group_name.to_string());
            }
        }

        tracing::warn!("{:?}", create_placement_group_response);
        anyhow::bail!("Failure finding the id of the created Placement Group resource");
    }

    pub async fn cleanup_placement_group(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_placement_groups_response = match context
            .ec2_client
            .describe_placement_groups()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing Placement Group resources: {}", e);
            }
        };

        let placement_groups = describe_placement_groups_response.placement_groups();
        if let Some(placement_group) = placement_groups.first() {
            if let Some(group_name) = placement_group.group_name() {
                tracing::info!("Found Placement Group to cleanup: '{}'", group_name);
                tracing::info!("Deleting Placement Group '{}'...", group_name);
                match context
                    .ec2_client
                    .delete_placement_group()
                    .group_name(group_name)
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!("Placement Group '{}' deleted successfully", group_name);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failure deleting placement Group resource: {}", e);
                    }
                }
            }
        }

        tracing::info!("No existing Placement Group found");
        Ok(())
    }
}
