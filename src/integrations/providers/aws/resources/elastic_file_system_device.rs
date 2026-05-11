use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;

impl AwsInterface {
    pub async fn request_elastic_file_system_device_creation(
        &self,
        context: &AwsClusterContext,
    ) -> Result<String> {
        let describe_efs_devices_response = match context
            .efs_client
            .describe_file_systems()
            .creation_token(context.cluster_id.as_str())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing EFS devices: {}", e);
            }
        };

        let efs_devices = describe_efs_devices_response.file_systems();
        match efs_devices.len() {
            0 => {
                tracing::info!("No existing EFS device found, continue with creation...")
            }
            1 => {
                let existing_efs_id = efs_devices[0].file_system_id().to_string();
                tracing::info!("Existing EFS device '{}' found", existing_efs_id);
                return Ok(existing_efs_id);
            }
            _ => {
                tracing::error!("{:?}", describe_efs_devices_response);
                anyhow::bail!(
                    "Unexpected multiple EFS devices found in AWS for cluster '{}'",
                    context.cluster_id
                );
            }
        }

        tracing::info!("Requesting creation of a new EFS device...");
        let performance_mode = match context.efs_performance_mode.as_str() {
            "max_io" => aws_sdk_efs::types::PerformanceMode::MaxIo,
            _ => aws_sdk_efs::types::PerformanceMode::GeneralPurpose,
        };
        let throughput_mode = match context.efs_throughput_mode.as_str() {
            "provisioned" => aws_sdk_efs::types::ThroughputMode::Provisioned,
            "elastic" => aws_sdk_efs::types::ThroughputMode::Elastic,
            _ => aws_sdk_efs::types::ThroughputMode::Bursting,
        };
        let mut efs_builder = context
            .efs_client
            .create_file_system()
            .creation_token(context.cluster_id.as_str())
            .set_availability_zone_name(if context.availability_zone.is_empty() {
                None
            } else {
                Some(context.availability_zone.clone())
            })
            .performance_mode(performance_mode)
            .throughput_mode(throughput_mode);
        if let Some(mbs) = context.efs_provisioned_throughput_mbs {
            efs_builder = efs_builder.provisioned_throughput_in_mibps(mbs);
        }
        let create_efs_device_response = match efs_builder
            .tags(
                aws_sdk_efs::types::Tag::builder()
                    .key("Name")
                    .value(context.efs_device_name.clone())
                    .build()
                    .unwrap(),
            )
            .tags(
                aws_sdk_efs::types::Tag::builder()
                    .key(context.cluster_id_tag.key().unwrap())
                    .value(context.cluster_id_tag.value().unwrap())
                    .build()
                    .unwrap(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure creating a new EFS device: {}", e);
            }
        };

        let new_efs_device_id = create_efs_device_response.file_system_id().to_string();
        if new_efs_device_id.is_empty() {
            tracing::error!("{:?}", create_efs_device_response);
            anyhow::bail!("Couldn't retrieve the existing EFS device id from AWS response");
        } else {
            tracing::info!(
                "Successfully requested creation of a new EFS device (id='{}')",
                new_efs_device_id
            );
        }

        tracing::info!(
            "Disabling automatic backups for EFS device (id='{}')...",
            new_efs_device_id
        );
        sleep(Duration::from_secs(10)).await;
        match context
            .efs_client
            .put_backup_policy()
            .file_system_id(&new_efs_device_id)
            .backup_policy(
                aws_sdk_efs::types::BackupPolicy::builder()
                    .status("DISABLED".into())
                    .build()
                    .unwrap(),
            )
            .send()
            .await
        {
            Ok(_) => {
                tracing::info!(
                    "Successfully disabled automatic backups for EFS device (id='{}')",
                    new_efs_device_id
                );
            }
            Err(e) => {
                tracing::error!("{:?}", e);
                tracing::warn!(
                    "Failure disabling automatic backups for EFS device (id='{}')",
                    new_efs_device_id
                );
            }
        };

        Ok(new_efs_device_id)
    }

    pub async fn wait_for_elastic_file_system_device_to_be_ready(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let efs_id = context.efs_device_id.clone().unwrap();
        tracing::info!("Waiting for EFS device (id='{}') to be ready...", efs_id);

        let max_wait_time = Duration::from_secs(300);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                let message = format!(
                    "Timeout waiting for EFS device (id='{}') to be available after {} seconds",
                    efs_id,
                    max_wait_time.as_secs()
                );
                tracing::warn!(message);
                anyhow::bail!(message);
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
                    tracing::error!("{:?}", e);
                    anyhow::bail!("Failure describing EFS devices: {}", e);
                }
            };

            let efs_devices = describe_efs_device_response.file_systems();
            if efs_devices.is_empty() {
                tracing::error!("{:?}", describe_efs_device_response);
                anyhow::bail!("Couldn't retrieve the existing EFS device id from AWS response");
            }

            let efs_device = &efs_devices[0];
            match efs_device.life_cycle_state() {
                aws_sdk_efs::types::LifeCycleState::Available => {
                    tracing::info!("EFS device (id='{}') is now available!", efs_id);
                    return Ok(());
                }
                _ => {
                    tracing::info!(
                        "EFS device (id='{}') is not available yet, (state='{}')...",
                        efs_id,
                        efs_device.life_cycle_state()
                    );
                }
            }

            sleep(poll_interval).await;
        }
    }

    pub async fn request_elastic_file_system_device_deletion(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let describe_file_system_response = match context
            .efs_client
            .describe_file_systems()
            .creation_token(context.cluster_id.as_str())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing EFS devices: {}", e);
            }
        };

        let file_systems = describe_file_system_response.file_systems();
        for file_system_description in file_systems {
            let efs_id = file_system_description.file_system_id().to_string();
            tracing::info!("Found EFS device to be deleted (id='{}')...", efs_id);
            match context
                .efs_client
                .delete_file_system()
                .file_system_id(&efs_id)
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!(
                        "Successfully requested deletion of Elastic File System (EFS) '{}'",
                        efs_id
                    );
                }
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure deleting Elastic File System (EFS) '{}' resource: {}",
                        efs_id,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn wait_for_elastic_file_system_device_to_be_deleted(
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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing EFS devices: {}", e);
            }
        };

        let efs_devices = describe_efs_devices_response.file_systems();
        if efs_devices.is_empty() {
            tracing::info!("No existing EFS device found, skip waiting for deletion...");
            return Ok(());
        }

        let efs_device_id = efs_devices[0].file_system_id();
        tracing::info!(
            "Waiting for EFS devices (id='{}') to be deleted...",
            efs_device_id
        );

        let max_wait_time = Duration::from_secs(300);
        let poll_interval = Duration::from_secs(10);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                let message = format!(
                    "Timeout waiting for EFS device (id='{}') to be deleted after {} seconds",
                    efs_device_id,
                    max_wait_time.as_secs()
                );
                tracing::warn!(message);
                anyhow::bail!(message);
            }

            let describe_efs_device_response = match context
                .efs_client
                .describe_file_systems()
                .file_system_id(efs_device_id)
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!(
                        "Failure describing EFS device (id='{}'): {}",
                        efs_device_id,
                        e
                    );
                }
            };

            let efs_devices = describe_efs_device_response.file_systems();
            if efs_devices.is_empty() {
                tracing::error!("{:?}", describe_efs_device_response);
                anyhow::bail!("Couldn't retrieve the existing EFS device id from AWS response");
            }

            let efs_device = &efs_devices[0];
            match efs_device.life_cycle_state() {
                aws_sdk_efs::types::LifeCycleState::Deleted => {
                    tracing::info!("EFS device (id='{}') is now deleted", efs_device_id);
                    return Ok(());
                }
                _ => {
                    tracing::info!(
                        "EFS device (id='{}') is not deleted yet, (state='{}')...",
                        efs_device_id,
                        efs_device.life_cycle_state()
                    );
                }
            }

            sleep(poll_interval).await;
        }
    }
}
