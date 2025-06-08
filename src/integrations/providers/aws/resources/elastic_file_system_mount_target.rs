use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::{Result, bail};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

impl AwsInterface {
    pub async fn request_elastic_file_system_mount_target_creation(
        &self,
        context: &AwsClusterContext,
    ) -> Result<String> {
        let efs_id = context.efs_device_id.clone().unwrap();
        let subnet_id = context.subnet_id.clone().unwrap();

        match context
            .efs_client
            .describe_mount_targets()
            .file_system_id(&efs_id)
            .send()
            .await
        {
            Ok(response) => {
                for mount_target_info in response.mount_targets() {
                    let mount_target_id = mount_target_info.mount_target_id().to_string();
                    if subnet_id == mount_target_info.subnet_id() {
                        info!(
                            "Found existing EFS mount target (id='{}') for EFS device (id='{}') in Subnet (id='{}')",
                            mount_target_id, efs_id, subnet_id
                        );
                        return Ok(mount_target_id);
                    } else {
                        warn!(
                            "There's an unexpected EFS mount target (id='{}') for EFS device (id='{}') in Subnet (id='{}')",
                            mount_target_id, efs_id, subnet_id
                        );
                    }
                }
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing existing EFS mount targets");
            }
        };

        let new_mount_target_id = match context
            .efs_client
            .create_mount_target()
            .file_system_id(efs_id.clone())
            .subnet_id(subnet_id.clone())
            .security_groups(&context.security_group_ids[0])
            .send()
            .await
        {
            Ok(response) => response.mount_target_id().to_string(),
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failure creating mount target in Subnet (id='{}') for EFS device (id='{}')",
                    subnet_id,
                    efs_id
                );
            }
        };

        Ok(new_mount_target_id)
    }

    pub async fn wait_for_elastic_file_system_mount_target_to_be_ready(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let efs_id = context.efs_device_id.clone().unwrap();
        info!(
            "Waiting for EFS mount target (id='{}') to be ready...",
            efs_id
        );

        let max_wait_time = Duration::from_secs(300);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                let message = format!(
                    "Timeout waiting for EFS target (id='{}') to be available after {} seconds",
                    efs_id,
                    max_wait_time.as_secs()
                );
                warn!(message);
                bail!(message);
            }

            let describe_efs_device_response = match context
                .efs_client
                .describe_file_systems()
                .file_system_id(&efs_id)
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure describing EFS devices");
                }
            };

            let efs_devices = describe_efs_device_response.file_systems();
            if efs_devices.is_empty() {
                error!("{:?}", describe_efs_device_response);
                bail!("Couldn't retrieve the existing EFS device id from AWS response");
            }

            let efs_device = &efs_devices[0];
            match efs_device.life_cycle_state() {
                aws_sdk_efs::types::LifeCycleState::Available => {
                    info!("EFS device (id='{}') is now available!", efs_id);
                    return Ok(());
                }
                _ => {
                    info!(
                        "EFS device (id='{}') is not available yet, (state='{}')...",
                        efs_id,
                        efs_device.life_cycle_state()
                    );
                }
            }

            sleep(poll_interval).await;
        }
    }

    pub async fn request_elastic_file_system_mount_target_deletion(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let describe_efs_devices_response = match context
            .efs_client
            .describe_file_systems()
            .creation_token(context.cluster_id.as_str())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failure describing EFS devices");
            }
        };

        let efs_devices = describe_efs_devices_response.file_systems();
        match efs_devices.len() {
            0 => {
                info!("No existing EFS device found, skip deletion...");
                return Ok(());
            }
            1 => {
                let existing_efs_id = efs_devices[0].file_system_id().to_string();
                info!(
                    "Existing EFS device (id='{}') found for deletion...",
                    existing_efs_id
                );
            }
            _ => {
                warn!("{:?}", describe_efs_devices_response);
                info!("Multiple existing EFS devices found for deletion...");
            }
        }

        for efs_device in efs_devices {
            let efs_device_id = efs_device.file_system_id();
            let describe_mount_targets_response = match context
                .efs_client
                .describe_mount_targets()
                .file_system_id(efs_device_id)
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!(
                        "Failure describing EFS mount targets for EFS device (id='{}')",
                        efs_device_id
                    );
                }
            };

            for mount_target in describe_mount_targets_response.mount_targets() {
                let mount_target_id = mount_target.mount_target_id();
                info!(
                    "Found EFS mount target (id='{}') for deletion...",
                    mount_target_id
                );
                match context
                    .efs_client
                    .delete_mount_target()
                    .mount_target_id(mount_target_id)
                    .send()
                    .await
                {
                    Ok(response) => response,
                    Err(e) => {
                        error!("{:?}", e);
                        bail!(
                            "Failure deleting EFS mount target (id='{}')",
                            mount_target_id
                        );
                    }
                };
            }
        }

        Ok(())
    }

    pub async fn wait_for_elastic_file_system_mount_target_to_be_deleted(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        info!(
            "Waiting for EFS mount target deletion to complete for cluster (id='{}')...",
            context.cluster_id
        );

        let max_wait_time = Duration::from_secs(300);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                let message = format!(
                    "Timeout waiting for EFS mount target deletion for cluster (id='{}') after {} seconds",
                    context.cluster_id,
                    max_wait_time.as_secs()
                );
                warn!(message);
                bail!(message);
            }

            let describe_efs_devices_response = match context
                .efs_client
                .describe_file_systems()
                .creation_token(context.cluster_id.as_str())
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failure describing EFS devices during deletion wait");
                }
            };

            let efs_devices = describe_efs_devices_response.file_systems();
            if efs_devices.is_empty() {
                info!(
                    "No EFS devices found for cluster (id='{}') - deletion complete",
                    context.cluster_id
                );
                return Ok(());
            }

            let mut any_mount_targets_remaining = false;

            for efs_device in efs_devices {
                let efs_device_id = efs_device.file_system_id();

                let describe_mount_targets_response = match context
                    .efs_client
                    .describe_mount_targets()
                    .file_system_id(efs_device_id)
                    .send()
                    .await
                {
                    Ok(response) => response,
                    Err(e) => {
                        error!("{:?}", e);
                        bail!(
                            "Failure describing EFS mount targets for EFS device (id='{}') during deletion wait",
                            efs_device_id
                        );
                    }
                };

                let mount_targets = describe_mount_targets_response.mount_targets();
                if !mount_targets.is_empty() {
                    any_mount_targets_remaining = true;
                    for mount_target in mount_targets {
                        let mount_target_id = mount_target.mount_target_id();
                        let state = mount_target.life_cycle_state();
                        info!(
                            "EFS mount target (id='{}') still exists with state '{}'",
                            mount_target_id, state
                        );
                    }
                }
            }

            if !any_mount_targets_remaining {
                info!(
                    "All EFS mount targets have been successfully deleted for cluster (id='{}')",
                    context.cluster_id
                );
                return Ok(());
            }

            sleep(poll_interval).await;
        }
    }
}
