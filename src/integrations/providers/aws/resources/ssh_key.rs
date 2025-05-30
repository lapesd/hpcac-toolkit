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
                bail!("Failure describing SSH key pair resources");
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                info!("Found existing SSH key pair: '{}'", key_id);

                // Verify the key name matches what we expect
                if let Some(key_name) = key_pair.key_name() {
                    if key_name == context.ssh_key_name {
                        info!(
                            "SSH key pair '{}' matches expected name '{}'",
                            key_id, key_name
                        );
                        return Ok(key_id.to_string());
                    } else {
                        warn!(
                            "SSH key pair '{}' has different name '{}', expected '{}'",
                            key_id, key_name, context.ssh_key_name
                        );
                    }
                }

                // If name doesn't match, we'll still use the existing key
                // since it was tagged with our cluster ID
                return Ok(key_id.to_string());
            }
        }

        info!("No existing SSH key pair found, importing a new one...");

        // Read the public key file
        let public_key_material = match fs::read_to_string(&context.public_ssh_key_path) {
            Ok(material) => material,
            Err(e) => {
                error!(
                    "Failed to read public key file from '{}': {:?}",
                    context.public_ssh_key_path, e
                );
                bail!("Failure reading public SSH key file");
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
                bail!("Failure importing SSH key pair");
            }
        };

        if let Some(key_id) = import_key_pair_response.key_pair_id() {
            info!("Successfully imported SSH key pair '{}'", key_id);
            Ok(key_id.to_string())
        } else {
            warn!("{:?}", import_key_pair_response);
            bail!("Unexpected response from AWS when importing SSH key pair");
        }
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
                bail!("Failure describing SSH key pair resources");
            }
        };

        let key_pairs = describe_key_pairs_response.key_pairs();
        if let Some(key_pair) = key_pairs.first() {
            if let Some(key_id) = key_pair.key_pair_id() {
                info!("Found SSH key pair to cleanup: '{}'", key_id);

                info!("Deleting SSH key pair '{}'...", key_id);
                match context
                    .client
                    .delete_key_pair()
                    .key_pair_id(key_id)
                    .send()
                    .await
                {
                    Ok(_) => {
                        info!("SSH key pair '{}' deleted successfully", key_id);
                        return Ok(());
                    }
                    Err(e) => {
                        error!("Failed to delete SSH key pair '{}': {:?}", key_id, e);
                        bail!("Failure deleting SSH key pair resource");
                    }
                }
            }
        }

        info!("No existing SSH key pair found to cleanup");
        Ok(())
    }
}
