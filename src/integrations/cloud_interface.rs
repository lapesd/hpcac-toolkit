use crate::commands::utils::ProgressTracker;
use crate::database::models::{
    CloudResource, Cluster, InstanceType, MachineImage, Node, ResourceStatus,
};
use crate::integrations::providers::{aws::AwsInterface, vultr::VultrInterface};
use anyhow::{Error, Result};
use std::collections::HashMap;
use std::process;
use tracing::{debug, error};

/// Trait for handling cloud errors - can be implemented by all cloud-related traits
pub trait CloudErrorHandler {
    fn handle_error<T>(&self, err: Error, message: &str) -> Result<T> {
        debug!("{}", err);
        error!("Cloud Operation Error: {}", message);
        process::exit(1);
    }
}

/// Trait for read-only information gathering operations
pub trait CloudInfoProvider: CloudErrorHandler {
    /// Fetch available regions from the cloud provider
    async fn fetch_regions(&self, tracker: &ProgressTracker) -> Result<Vec<String>, Error>;

    /// Fetch available zones within a specific region
    async fn fetch_zones(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<String>, Error>;

    /// Fetch available instance types in a specific region
    async fn fetch_instance_types(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>, Error>;

    /// Fetch pricing information for specific instance types in a region
    async fn fetch_prices(
        &self,
        region: &str,
        instance_types: &[String],
        tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>, Error>;

    /// Fetch details about a specific machine image
    async fn fetch_machine_image(
        &self,
        region: &str,
        image_id: &str,
    ) -> Result<MachineImage, Error>;
}

/// Trait for managing cloud resources lifecycle
pub trait CloudResourceManager: CloudErrorHandler {
    /// Create a new cluster with the specified nodes
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error>;

    /// Check if a cluster with the given ID exists
    async fn check_cluster_exists(&self, cluster_id: &str) -> Result<bool, Error>;

    /// Delete a cluster and all its associated resources
    async fn delete_cluster(&self, cluster_id: &str) -> Result<(), Error>;

    /// Find and clean up any orphaned resources associated with a cluster
    async fn cleanup_orphaned_resources(&self, cluster_id: &str) -> Result<Vec<String>, Error>;
}

pub trait CloudResourceTracker: CloudErrorHandler {
    async fn list_resources_by_cluster(
        &self,
        cluster_id: &str,
    ) -> Result<Vec<CloudResource>, Error>;

    async fn get_resource_status(&self, resource_id: &str) -> Result<ResourceStatus, Error>;
}

pub enum CloudProvider {
    Aws(AwsInterface),
    Vultr(VultrInterface),
}

impl CloudErrorHandler for CloudProvider {
    fn handle_error<T>(&self, err: Error, message: &str) -> Result<T> {
        match self {
            CloudProvider::Aws(aws) => aws.handle_error(err, message),
            CloudProvider::Vultr(vultr) => vultr.handle_error(err, message),
        }
    }
}

impl CloudInfoProvider for CloudProvider {
    async fn fetch_regions(&self, tracker: &ProgressTracker) -> Result<Vec<String>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_regions(tracker).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_regions(tracker).await,
        }
    }

    async fn fetch_zones(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<String>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_zones(region, tracker).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_zones(region, tracker).await,
        }
    }

    async fn fetch_instance_types(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_instance_types(region, tracker).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_instance_types(region, tracker).await,
        }
    }

    async fn fetch_prices(
        &self,
        region: &str,
        instance_types: &[String],
        tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_prices(region, instance_types, tracker).await,
            CloudProvider::Vultr(vultr) => {
                vultr.fetch_prices(region, instance_types, tracker).await
            }
        }
    }

    async fn fetch_machine_image(
        &self,
        region: &str,
        image_id: &str,
    ) -> Result<MachineImage, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_machine_image(region, image_id).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_machine_image(region, image_id).await,
        }
    }
}

impl CloudResourceManager for CloudProvider {
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.spawn_cluster(cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.spawn_cluster(cluster, nodes).await,
        }
    }

    async fn check_cluster_exists(&self, cluster_id: &str) -> Result<bool, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.check_cluster_exists(cluster_id).await,
            CloudProvider::Vultr(vultr) => vultr.check_cluster_exists(cluster_id).await,
        }
    }

    async fn delete_cluster(&self, cluster_id: &str) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.delete_cluster(cluster_id).await,
            CloudProvider::Vultr(vultr) => vultr.delete_cluster(cluster_id).await,
        }
    }

    async fn cleanup_orphaned_resources(&self, cluster_id: &str) -> Result<Vec<String>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.cleanup_orphaned_resources(cluster_id).await,
            CloudProvider::Vultr(vultr) => vultr.cleanup_orphaned_resources(cluster_id).await,
        }
    }
}

impl CloudResourceTracker for CloudProvider {
    async fn list_resources_by_cluster(
        &self,
        cluster_id: &str,
    ) -> Result<Vec<CloudResource>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.list_resources_by_cluster(cluster_id).await,
            CloudProvider::Vultr(vultr) => vultr.list_resources_by_cluster(cluster_id).await,
        }
    }

    async fn get_resource_status(&self, resource_id: &str) -> Result<ResourceStatus, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.get_resource_status(resource_id).await,
            CloudProvider::Vultr(vultr) => vultr.get_resource_status(resource_id).await,
        }
    }
}
