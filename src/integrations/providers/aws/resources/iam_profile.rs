use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use aws_sdk_iam::error::SdkError;
use aws_sdk_iam::operation::get_instance_profile::GetInstanceProfileError;
use tracing::{error, info};

impl AwsInterface {
    pub async fn ensure_iam_profile(&self, context: &AwsClusterContext) -> Result<String> {
        let profile_name = context.iam_profile_name.clone();
        let role_name = context.iam_role_name.clone();

        match context
            .iam_client
            .get_instance_profile()
            .instance_profile_name(&profile_name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(profile) = response.instance_profile() {
                    let iam_profile_id = profile.instance_profile_id();
                    info!(
                        "Found existing IAM Profile (id='{}'), skipping creation...",
                        iam_profile_id
                    );
                    return Ok(iam_profile_id.to_string());
                }
            }
            Err(SdkError::ServiceError(service_err)) => match service_err.err() {
                GetInstanceProfileError::NoSuchEntityException(_) => {
                    info!(
                        "IAM Profile (name='{}') does not exist, will create it",
                        profile_name
                    );
                }
                _ => {
                    error!("{:?}", service_err);
                    bail!("Failure describing IAM Profile (name='{}')", profile_name);
                }
            },
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing IAM Profile (name='{}')", profile_name);
            }
        };

        let create_iam_profile_response = match context
            .iam_client
            .create_instance_profile()
            .instance_profile_name(&profile_name)
            .tags(
                aws_sdk_iam::types::Tag::builder()
                    .key("Name")
                    .value(&profile_name)
                    .build()
                    .unwrap(),
            )
            .tags(
                aws_sdk_iam::types::Tag::builder()
                    .key(context.cluster_id_tag.key().unwrap())
                    .value(context.cluster_id_tag.value().unwrap())
                    .build()
                    .unwrap(),
            )
            .send()
            .await
        {
            Ok(response) => {
                info!("Created IAM Profile (name='{}')", profile_name);
                response
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to create IAM Profile (name='{}')", profile_name);
            }
        };

        match context
            .iam_client
            .add_role_to_instance_profile()
            .instance_profile_name(&profile_name)
            .role_name(&role_name)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "Successfully assumed IAM Role (name='{}') in IAM Profile (name='{}')",
                    role_name, profile_name
                );
            }
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to assume IAM Role (name='{}') in IAM Profile (name='{}')",
                    role_name,
                    profile_name,
                );
            }
        }

        let iam_profile_id = create_iam_profile_response
            .instance_profile()
            .unwrap()
            .instance_profile_id()
            .to_string();

        Ok(iam_profile_id)
    }

    pub async fn cleanup_iam_profile(&self, context: &AwsClusterContext) -> Result<()> {
        let profile_name = context.iam_profile_name.clone();
        info!("Cleaning up IAM Profile (name='{}')...", profile_name);

        match context
            .iam_client
            .get_instance_profile()
            .instance_profile_name(&profile_name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(instance_profile) = response.instance_profile() {
                    let iam_profile_id = instance_profile.instance_profile_id();
                    info!(
                        "Found existing IAM Profile (id='{}'), proceeding with deletion...",
                        iam_profile_id
                    );

                    for role in instance_profile.roles() {
                        let role_name = role.role_name();
                        let _remove_role_response = match context
                            .iam_client
                            .remove_role_from_instance_profile()
                            .instance_profile_name(&profile_name)
                            .role_name(role_name)
                            .send()
                            .await
                        {
                            Ok(response) => {
                                info!(
                                    "Removed IAM Role (name='{}') from IAM Profile (name='{}')",
                                    role_name, profile_name
                                );
                                response
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                bail!(
                                    "Failure removing IAM Role (name='{}') from IAM Profile (name='{}')",
                                    role_name,
                                    profile_name
                                );
                            }
                        };
                    }
                } else {
                    info!(
                        "IAM Profile (name='{}') does not exist, skipping deletion...",
                        profile_name
                    );
                    return Ok(());
                }
            }
            Err(SdkError::ServiceError(service_err)) => match service_err.err() {
                GetInstanceProfileError::NoSuchEntityException(_) => {
                    info!(
                        "IAM Profile (name='{}') does not exist, skipping deletion...",
                        profile_name
                    );
                    return Ok(());
                }
                _ => {
                    error!("{:?}", service_err);
                    bail!("Failure describing IAM Profile '{}'", profile_name);
                }
            },
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing IAM Profile '{}'", profile_name);
            }
        }

        let _delete_instance_profile_response = match context
            .iam_client
            .delete_instance_profile()
            .instance_profile_name(&profile_name)
            .send()
            .await
        {
            Ok(response) => {
                info!("Successfully deleted IAM Profile (name='{}')", profile_name);
                response
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to delete IAM Profile (name='{}')", profile_name);
            }
        };

        Ok(())
    }
}
