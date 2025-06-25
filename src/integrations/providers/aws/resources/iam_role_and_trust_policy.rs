use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use aws_sdk_iam::error::SdkError;
use aws_sdk_iam::operation::get_role::GetRoleError;
use tracing::{error, info};

impl AwsInterface {
    pub async fn ensure_iam_role_and_trust_policies(
        &self,
        context: &AwsClusterContext,
    ) -> Result<String> {
        let role_name = context.iam_role_name.clone();

        match context
            .iam_client
            .get_role()
            .role_name(&role_name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(role) = response.role() {
                    let iam_role_id = role.role_id();
                    info!(
                        "Found existing IAM Role (id='{}'), skipping creation...",
                        iam_role_id
                    );
                    return Ok(iam_role_id.to_string());
                }
            }
            Err(SdkError::ServiceError(service_err)) => match service_err.err() {
                GetRoleError::NoSuchEntityException(_) => {
                    info!(
                        "IAM Role (name='{}') does not exist, will create it",
                        role_name
                    );
                }
                _ => {
                    error!("{:?}", service_err);
                    bail!("Failure describing IAM Role (name='{}')", role_name);
                }
            },
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing IAM Role (name='{}')", role_name);
            }
        };

        let trust_policy = r#"{
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Principal": {
                        "Service": [
                            "ec2.amazonaws.com",
                            "ssm.amazonaws.com"
                        ]
                    },
                    "Action": "sts:AssumeRole"
                }
            ]
        }"#;

        let create_iam_role_response = match context
            .iam_client
            .create_role()
            .role_name(&role_name)
            .assume_role_policy_document(trust_policy)
            .description("Role for EC2 instances to use Systems Manager")
            .tags(
                aws_sdk_iam::types::Tag::builder()
                    .key("Name")
                    .value(&role_name)
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
                info!("Created IAM role (name='{}')", role_name);
                response
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to create IAM role (name='{}')", role_name);
            }
        };

        match context
            .iam_client
            .attach_role_policy()
            .role_name(&role_name)
            .policy_arn("arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore")
            .send()
            .await
        {
            Ok(_) => {
                info!("Attached SSM Policy to IAM Role '{}'", role_name);
            }
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to attach SSM Policy to IAM Role (name='{}')",
                    role_name
                );
            }
        }

        let role_id = create_iam_role_response
            .role()
            .unwrap()
            .role_id()
            .to_string();

        Ok(role_id)
    }

    pub async fn cleanup_trust_policies_and_iam_role(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let role_name = context.iam_role_name.clone();
        info!("Cleaning up IAM Role (name='{}')...", role_name);

        match context
            .iam_client
            .get_role()
            .role_name(&role_name)
            .send()
            .await
        {
            Ok(response) => {
                if let Some(role) = response.role() {
                    let iam_role_id = role.role_id();
                    info!(
                        "Found existing IAM Role (id='{}'), proceeding with deletion...",
                        iam_role_id
                    );
                } else {
                    info!(
                        "IAM Role (name='{}') does not exist, skipping deletion...",
                        role_name
                    );
                    return Ok(());
                }
            }
            Err(SdkError::ServiceError(service_err)) => match service_err.err() {
                GetRoleError::NoSuchEntityException(_) => {
                    info!(
                        "IAM Role (name='{}') does not exist, skipping deletion...",
                        role_name
                    );
                    return Ok(());
                }
                _ => {
                    error!("{:?}", service_err);
                    bail!("Failure describing IAM Role '{}'", role_name);
                }
            },
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing IAM Role '{}'", role_name);
            }
        }

        let _detach_ssm_policy_from_iam_role_response = match context
            .iam_client
            .detach_role_policy()
            .role_name(&role_name)
            .policy_arn("arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore")
            .send()
            .await
        {
            Ok(response) => {
                info!(
                    "Detached SSM Trust Policy from IAM Role (name='{}')",
                    role_name
                );
                response
            }
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failure detaching SSM Trust Policy from IAM Role (name='{}')",
                    role_name
                );
            }
        };

        let _delete_iam_role_response = match context
            .iam_client
            .delete_role()
            .role_name(&role_name)
            .send()
            .await
        {
            Ok(response) => {
                info!("Successfully deleted IAM Role (name='{}')", role_name);
                response
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to delete IAM Role (name='{}')", role_name);
            }
        };

        Ok(())
    }
}
