use crate::database::models::{Cluster, InstanceType, MachineImage, Node};
use crate::integrations::providers::{aws::AwsInterface, vultr::VultrInterface};
use crate::utils::ProgressTracker;

use anyhow::{Error, Result};
use std::collections::HashMap;
use tracing::error;

/// Trait for handling cloud errors - can be implemented by all cloud-related traits
pub trait CloudErrorHandler {
    fn handle_error<T>(&self, e: Error, message: &str) -> Result<T> {
        error!("{}", e);
        anyhow::bail!("Cloud Operation: {}", message);
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

    /// Delete a cluster and all its associated resources
    async fn destroy_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error>;
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

    async fn destroy_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.destroy_cluster(cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.destroy_cluster(cluster, nodes).await,
        }
    }
}
