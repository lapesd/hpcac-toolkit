use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use tracing::{error, info, warn};

impl AwsInterface {
    async fn _check_if_amazon_machine_image_is_available(
        &self,
        context: &AwsClusterContext,
        image_id: &str,
    ) -> Result<()> {
        let describe_machine_image_response = match context
            .ec2_client
            .describe_images()
            .image_ids(image_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to describe Amazon Machine Image (AMI) (id='{}')",
                    image_id,
                )
            }
        };

        let image = &describe_machine_image_response.images()[0];
        match image.state() {
            Some(aws_sdk_ec2::types::ImageState::Available) => {
                info!(
                    "Amazon Machine Image (AMI) (id='{}') is available",
                    image_id
                );
                Ok(())
            }
            Some(state) => {
                warn!("AMI state: {:?}", state);
                bail!(
                    "Amazon Machine Image (AMI) (id='{}') is not in an available state (current state: {:?})",
                    image_id,
                    state
                );
            }
            None => {
                warn!("{:?}", image);
                bail!(
                    "Amazon Machine Image (AMI) (id='{}') has unknown state",
                    image_id
                );
            }
        }
    }
}
