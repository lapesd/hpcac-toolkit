use crate::database::models::{Cluster, InstanceType, MachineImage, Node};
use crate::integrations::providers::{aws::AwsInterface, gcp::GcpInterface, vultr::VultrInterface};
use crate::utils::ProgressTracker;

use anyhow::{Error, Result};
use sqlx::sqlite::SqlitePool;
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
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<(), Error>;
    async fn terminate_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<(), Error>;
    async fn simulate_cluster_failure(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<(), Error>;
}

pub enum CloudProvider {
    Aws(AwsInterface),
    Vultr(VultrInterface),
    Gcp(GcpInterface),
}

impl CloudInfoProvider for CloudProvider {
    async fn fetch_regions(&self, tracker: &ProgressTracker) -> Result<Vec<String>, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_regions(tracker).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_regions(tracker).await,
            CloudProvider::Gcp(gcp) => gcp.fetch_regions(tracker).await,
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
            CloudProvider::Gcp(gcp) => gcp.fetch_zones(region, tracker).await,
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
            CloudProvider::Gcp(gcp) => gcp.fetch_instance_types(region, tracker).await,
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
            CloudProvider::Gcp(gcp) => gcp.fetch_prices(region, instance_types, tracker).await,
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
            CloudProvider::Gcp(gcp) => gcp.fetch_machine_image(region, image_id).await,
        }
    }
}

impl CloudResourceManager for CloudProvider {
    async fn spawn_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.spawn_cluster(pool, cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.spawn_cluster(pool, cluster, nodes).await,
            CloudProvider::Gcp(gcp) => gcp.spawn_cluster(pool, cluster, nodes).await,
        }
    }

    async fn terminate_cluster(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        nodes: Vec<Node>,
    ) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.terminate_cluster(pool, cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.terminate_cluster(pool, cluster, nodes).await,
            CloudProvider::Gcp(gcp) => gcp.terminate_cluster(pool, cluster, nodes).await,
        }
    }

    async fn simulate_cluster_failure(
        &self,
        pool: &SqlitePool,
        cluster: Cluster,
        node_private_ip: &str,
    ) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => {
                aws.simulate_cluster_failure(pool, cluster, node_private_ip)
                    .await
            }
            CloudProvider::Vultr(vultr) => {
                vultr
                    .simulate_cluster_failure(pool, cluster, node_private_ip)
                    .await
            }
            CloudProvider::Gcp(gcp) => {
                gcp.simulate_cluster_failure(pool, cluster, node_private_ip)
                    .await
            }
        }
    }
}
