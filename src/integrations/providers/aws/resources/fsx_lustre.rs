use crate::integrations::providers::aws::{AwsInterface, interface::AwsClusterContext};

use anyhow::Result;
use aws_sdk_ec2::types::SubnetCidrReservationType;
use aws_sdk_fsx::types::{
    CreateFileSystemLustreConfiguration, DeleteFileSystemLustreConfiguration, FileSystem,
    FileSystemLifecycle, FileSystemType, LustreDeploymentType, Tag,
};
use std::time::Duration;
use tokio::time::sleep;

/// Lustre version of the file system. It must match the client that Amazon Linux 2023
/// installs from its `lustre-client` package.
const LUSTRE_VERSION: &str = "2.15";

/// Node ENIs use fixed addresses from 10.0.0.10 upwards. FSx places its own network
/// interfaces on addresses AWS picks from the subnet, and unlike an EFS mount target
/// they cannot be pinned, so this range is reserved to keep them out of it.
const NODE_IP_RESERVATION_CIDR: &str = "10.0.0.0/25";

impl AwsInterface {
    /// Finds the cluster's file system by its tag, ignoring any that are going away.
    async fn find_fsx_lustre_file_system(
        &self,
        context: &AwsClusterContext,
    ) -> Result<Option<FileSystem>> {
        let cluster_id_key = context.cluster_id_tag.key().unwrap();
        let mut next_token = None;
        loop {
            let response = match context
                .fsx_client
                .describe_file_systems()
                .set_next_token(next_token)
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!("Failure describing FSx file systems: {}", e);
                }
            };

            for file_system in response.file_systems() {
                let belongs_to_cluster = file_system.tags().iter().any(|tag| {
                    tag.key() == Some(cluster_id_key)
                        && tag.value() == Some(context.cluster_id.as_str())
                });
                let is_gone = matches!(
                    file_system.lifecycle(),
                    Some(FileSystemLifecycle::Deleting) | Some(FileSystemLifecycle::Failed)
                );
                if belongs_to_cluster && !is_gone {
                    return Ok(Some(file_system.clone()));
                }
            }

            next_token = response.next_token().map(str::to_string);
            if next_token.is_none() {
                return Ok(None);
            }
        }
    }

    /// Keeps FSx network interfaces off the addresses that nodes use. The reservation
    /// is removed along with the subnet.
    pub async fn ensure_fsx_lustre_ip_reservation(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let subnet_id = context.subnet_id.clone().unwrap();
        let existing = match context
            .ec2_client
            .get_subnet_cidr_reservations()
            .subnet_id(&subnet_id)
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure describing Subnet CIDR reservations: {}", e);
            }
        };
        if existing
            .subnet_ipv4_cidr_reservations()
            .iter()
            .any(|reservation| reservation.cidr() == Some(NODE_IP_RESERVATION_CIDR))
        {
            tracing::info!(
                "Node IP range is already reserved in Subnet '{}'",
                subnet_id
            );
            return Ok(());
        }

        match context
            .ec2_client
            .create_subnet_cidr_reservation()
            .subnet_id(&subnet_id)
            .cidr(NODE_IP_RESERVATION_CIDR)
            .reservation_type(SubnetCidrReservationType::Explicit)
            .description("Node ENI addresses, kept clear of FSx network interfaces")
            .send()
            .await
        {
            Ok(_) => {
                tracing::info!(
                    "Reserved '{}' for nodes in Subnet '{}'",
                    NODE_IP_RESERVATION_CIDR,
                    subnet_id
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure reserving the node IP range: {}", e);
            }
        }
    }

    pub async fn request_fsx_lustre_file_system_creation(
        &self,
        context: &AwsClusterContext,
    ) -> Result<String> {
        if let Some(existing) = self.find_fsx_lustre_file_system(context).await? {
            let existing_id = existing.file_system_id().unwrap_or_default().to_string();
            tracing::info!(
                "Existing FSx for Lustre file system '{}' found",
                existing_id
            );
            return Ok(existing_id);
        }

        tracing::info!("Requesting creation of a new FSx for Lustre file system...");
        let is_persistent = context.fsx_deployment_type == "persistent_2";
        let mut lustre_configuration = CreateFileSystemLustreConfiguration::builder()
            .deployment_type(if is_persistent {
                LustreDeploymentType::Persistent2
            } else {
                LustreDeploymentType::Scratch2
            });
        if is_persistent {
            lustre_configuration = lustre_configuration
                .set_per_unit_storage_throughput(
                    context.fsx_per_unit_storage_throughput.map(|t| t as i32),
                )
                .automatic_backup_retention_days(0);
        }

        let response = match context
            .fsx_client
            .create_file_system()
            .file_system_type(FileSystemType::Lustre)
            .file_system_type_version(LUSTRE_VERSION)
            .storage_capacity(context.fsx_storage_capacity_gib as i32)
            .subnet_ids(context.subnet_id.clone().unwrap())
            .security_group_ids(&context.security_group_ids[0])
            .lustre_configuration(lustre_configuration.build())
            .tags(
                Tag::builder()
                    .key("Name")
                    .value(context.fsx_file_system_name.clone())
                    .build(),
            )
            .tags(
                Tag::builder()
                    .key(context.cluster_id_tag.key().unwrap())
                    .value(context.cluster_id.clone())
                    .build(),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!("Failure creating a new FSx for Lustre file system: {}", e);
            }
        };

        let Some(new_id) = response.file_system().and_then(|fs| fs.file_system_id()) else {
            tracing::error!("{:?}", response);
            anyhow::bail!("Couldn't retrieve the FSx file system id from AWS response");
        };
        tracing::info!("Requested FSx for Lustre file system (id='{}')", new_id);
        Ok(new_id.to_string())
    }

    /// Waits until the file system is available and returns its DNS and mount names.
    pub async fn wait_for_fsx_lustre_file_system_to_be_ready(
        &self,
        context: &AwsClusterContext,
    ) -> Result<(String, String)> {
        let file_system_id = context.fsx_file_system_id.clone().unwrap();
        // Creation usually takes minutes, but AWS gives no upper bound.
        let max_wait_time = Duration::from_secs(1800);
        let poll_interval = Duration::from_secs(20);
        let start_time = std::time::Instant::now();

        loop {
            if start_time.elapsed() >= max_wait_time {
                anyhow::bail!(
                    "Timeout waiting for FSx file system (id='{}') after {} seconds",
                    file_system_id,
                    max_wait_time.as_secs()
                );
            }

            let response = match context
                .fsx_client
                .describe_file_systems()
                .file_system_ids(&file_system_id)
                .send()
                .await
            {
                Ok(response) => response,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    anyhow::bail!("Failure describing FSx file system: {}", e);
                }
            };
            let Some(file_system) = response.file_systems().first() else {
                anyhow::bail!("FSx file system (id='{}') not found", file_system_id);
            };

            match file_system.lifecycle() {
                Some(FileSystemLifecycle::Available) => {
                    let dns_name = file_system.dns_name().unwrap_or_default().to_string();
                    let mount_name = file_system
                        .lustre_configuration()
                        .and_then(|lustre| lustre.mount_name())
                        .unwrap_or_default()
                        .to_string();
                    if dns_name.is_empty() || mount_name.is_empty() {
                        tracing::error!("{:?}", file_system);
                        anyhow::bail!(
                            "FSx file system (id='{}') has no DNS or mount name",
                            file_system_id
                        );
                    }
                    tracing::info!("FSx file system (id='{}') is available", file_system_id);
                    return Ok((dns_name, mount_name));
                }
                Some(FileSystemLifecycle::Failed) | Some(FileSystemLifecycle::Misconfigured) => {
                    let reason = file_system
                        .failure_details()
                        .and_then(|details| details.message())
                        .unwrap_or("no details given");
                    anyhow::bail!(
                        "FSx file system (id='{}') failed: {}",
                        file_system_id,
                        reason
                    );
                }
                state => {
                    tracing::info!(
                        "FSx file system (id='{}') is not available yet (state='{:?}')...",
                        file_system_id,
                        state
                    );
                }
            }

            sleep(poll_interval).await;
        }
    }

    pub async fn request_fsx_lustre_file_system_deletion(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let Some(file_system) = self.find_fsx_lustre_file_system(context).await? else {
            tracing::info!("No existing FSx file system found, skip deletion...");
            return Ok(());
        };
        let file_system_id = file_system.file_system_id().unwrap_or_default();

        let mut request = context
            .fsx_client
            .delete_file_system()
            .file_system_id(file_system_id);
        // Without this a persistent file system bills for a final backup after deletion.
        if context.fsx_deployment_type == "persistent_2" {
            request = request.lustre_configuration(
                DeleteFileSystemLustreConfiguration::builder()
                    .skip_final_backup(true)
                    .build(),
            );
        }
        match request.send().await {
            Ok(_) => {
                tracing::info!("Requested deletion of FSx file system '{}'", file_system_id);
                Ok(())
            }
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failure deleting FSx file system '{}': {}",
                    file_system_id,
                    e
                );
            }
        }
    }

    /// The subnet and security group can only go once the file system's network
    /// interfaces have, so this must finish before they are destroyed.
    pub async fn wait_for_fsx_lustre_file_system_to_be_deleted(
        &self,
        context: &AwsClusterContext,
    ) -> Result<()> {
        let max_wait_time = Duration::from_secs(1800);
        let poll_interval = Duration::from_secs(20);
        let start_time = std::time::Instant::now();
        let cluster_id_key = context.cluster_id_tag.key().unwrap();

        loop {
            if start_time.elapsed() >= max_wait_time {
                anyhow::bail!(
                    "Timeout waiting for the FSx file system of cluster '{}' to be deleted",
                    context.cluster_id
                );
            }

            let mut remaining = false;
            let mut next_token = None;
            loop {
                let response = match context
                    .fsx_client
                    .describe_file_systems()
                    .set_next_token(next_token)
                    .send()
                    .await
                {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        anyhow::bail!("Failure describing FSx file systems: {}", e);
                    }
                };
                remaining |= response.file_systems().iter().any(|file_system| {
                    file_system.tags().iter().any(|tag| {
                        tag.key() == Some(cluster_id_key)
                            && tag.value() == Some(context.cluster_id.as_str())
                    })
                });
                next_token = response.next_token().map(str::to_string);
                if next_token.is_none() {
                    break;
                }
            }

            if !remaining {
                tracing::info!(
                    "FSx file system of cluster '{}' is deleted",
                    context.cluster_id
                );
                return Ok(());
            }
            tracing::info!(
                "FSx file system of cluster '{}' is still deleting...",
                context.cluster_id
            );
            sleep(poll_interval).await;
        }
    }
}
