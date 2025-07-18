use crate::database::models::{Cluster, InstanceType, MachineImage, Node};
use crate::integrations::providers::{aws::AwsInterface, vultr::VultrInterface};
use crate::utils::ProgressTracker;

use anyhow::{Error, Result};
use std::collections::HashMap;

pub trait CloudInfoProvider {
    async fn fetch_regions(&self, tracker: &ProgressTracker) -> Result<Vec<String>, Error>;

    async fn fetch_zones(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<String>, Error>;

    async fn fetch_instance_types(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>, Error>;

    async fn fetch_prices(
        &self,
        region: &str,
        instance_types: &[String],
        tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>, Error>;

    async fn fetch_machine_image(
        &self,
        region: &str,
        image_id: &str,
    ) -> Result<MachineImage, Error>;
}

pub trait CloudResourceManager {
    async fn spawn_cluster(
        &self,
        cluster: Cluster,
        nodes: Vec<Node>,
        init_commands: HashMap<usize, Vec<String>>,
    ) -> Result<(), Error>;
    async fn terminate_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error>;
    async fn simulate_cluster_failure(
        &self,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<(), Error>;
}

pub enum CloudProvider {
    Aws(AwsInterface),
    Vultr(VultrInterface),
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
    async fn spawn_cluster(
        &self,
        cluster: Cluster,
        nodes: Vec<Node>,
        init_commands: HashMap<usize, Vec<String>>,
    ) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.spawn_cluster(cluster, nodes, init_commands).await,
            CloudProvider::Vultr(vultr) => vultr.spawn_cluster(cluster, nodes, init_commands).await,
        }
    }

    async fn terminate_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.terminate_cluster(cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.terminate_cluster(cluster, nodes).await,
        }
    }

    async fn simulate_cluster_failure(
        &self,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.simulate_cluster_failure(cluster, node_private_ip).await,
            CloudProvider::Vultr(vultr) => {
                vultr
                    .simulate_cluster_failure(cluster, node_private_ip)
                    .await
            }
        }
    }
}
