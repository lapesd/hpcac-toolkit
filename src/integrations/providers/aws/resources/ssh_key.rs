use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;
use std::fs;

impl AwsInterface {
    pub async fn ensure_ssh_key(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_key_pairs_response = match context
            .ec2_client
            .describe_key_pairs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing SSH Keys resources: {}", e);
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                tracing::info!("Found existing SSH Key: '{}'", key_id);
                return Ok(key_id.to_string());
            }
        }

        tracing::info!("No existing SSH Key found, importing a new one...");

        let public_key_material = match fs::read_to_string(&context.public_ssh_key_path) {
            Ok(material) => material,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failure reading public SSH Key file from '{}'",
                    context.public_ssh_key_path,
                );
            }
        };

        let import_key_pair_response = match context
            .ec2_client
            .import_key_pair()
            .key_name(context.ssh_key_name.clone())
            .public_key_material(aws_sdk_ec2::primitives::Blob::new(
                public_key_material.as_bytes(),
            ))
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::KeyPair)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(context.ssh_key_name.clone())
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
                anyhow::bail!("Failure importing SSH Key pair: {}", e);
            }
        };

        if let Some(key_id) = import_key_pair_response.key_pair_id() {
            tracing::info!("Successfully imported SSH Key '{}'", key_id);
            return Ok(key_id.to_string());
        }

        tracing::warn!("{:?}", import_key_pair_response);
        anyhow::bail!("Failure finding the id of the created SSH Key resource");
    }

    pub async fn cleanup_ssh_key(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_key_pairs_response = match context
            .ec2_client
            .describe_key_pairs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing SSH Key resources: {}", e);
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                tracing::info!("Found SSH Key to cleanup: '{}'", key_id);

                tracing::info!("Deleting SSH Key '{}'...", key_id);
                match context
                    .ec2_client
                    .delete_key_pair()
                    .key_pair_id(key_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        tracing::info!("SSH Key '{}' deleted successfully", key_id);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("Failed to delete SSH Key '{}': {:?}", key_id, e);
                        anyhow::bail!("Failure deleting SSH Key resource: {}", e);
                    }
                }
            }
        }

        tracing::info!("No existing SSH Key found");
        Ok(())
    }
}
