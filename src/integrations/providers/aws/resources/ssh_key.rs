use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use std::fs;
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn ensure_ssh_key(&self, context: &AwsClusterContext) -> Result<String> {
        let describe_key_pairs_response = match context
            .client
            .describe_key_pairs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing SSH Keys resources");
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                info!("Found existing SSH Key: '{}'", key_id);
                return Ok(key_id.to_string());
            }
        }

        info!("No existing SSH Key found, importing a new one...");

        let public_key_material = match fs::read_to_string(&context.public_ssh_key_path) {
            Ok(material) => material,
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failure reading public SSH Key file from '{}'",
                    context.public_ssh_key_path,
                );
            }
        };

        let import_key_pair_response = match context
            .client
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
                error!("{:?}", e);
                bail!("Failure importing SSH Key pair");
            }
        };

        if let Some(key_id) = import_key_pair_response.key_pair_id() {
            info!("Successfully imported SSH Key '{}'", key_id);
            return Ok(key_id.to_string());
        }

        warn!("{:?}", import_key_pair_response);
        bail!("Failure finding the id of the created SSH Key resource");
    }

    pub async fn cleanup_ssh_key(&self, context: &AwsClusterContext) -> Result<()> {
        let describe_key_pairs_response = match context
            .client
            .describe_key_pairs()
            .filters(context.cluster_id_filter.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing SSH Key resources");
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                info!("Found SSH Key to cleanup: '{}'", key_id);

                info!("Deleting SSH Key '{}'...", key_id);
                match context
                    .client
                    .delete_key_pair()
                    .key_pair_id(key_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("SSH Key '{}' deleted successfully", key_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Failed to delete SSH Key '{}': {:?}", key_id, e);
                        bail!("Failure deleting SSH Key resource");
                    }
                }
            }
        }

        info!("No existing SSH Key found");
        Ok(())
    }
}
