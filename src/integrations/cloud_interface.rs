use crate::commands::utils::ProgressTracker;
use crate::database::models::{Cluster, InstanceType, Node};
use crate::integrations::{
    data_transfer_objects::MachineImageDetail,
    providers::{aws::AwsInterface, vultr::VultrInterface},
};
use anyhow::{Error, Result};
use std::collections::HashMap;
use std::process;
use tracing::{debug, error};

pub trait CloudInterface {
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
    ) -> Result<MachineImageDetail, Error>;
    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error>;
    fn handle_error<T>(&self, err: Error, message: &str) -> Result<T> {
        debug!("{}", err);
        error!("Cloud Operation Error: {}", message);
        process::exit(1);
    }
}

pub enum CloudProvider {
    Aws(AwsInterface),
    Vultr(VultrInterface),
}

impl CloudInterface for CloudProvider {
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
    ) -> Result<MachineImageDetail, Error> {
        match self {
            CloudProvider::Aws(aws) => aws.fetch_machine_image(region, image_id).await,
            CloudProvider::Vultr(vultr) => vultr.fetch_machine_image(region, image_id).await,
        }
    }

    async fn spawn_cluster(&self, cluster: Cluster, nodes: Vec<Node>) -> Result<(), Error> {
        match self {
            CloudProvider::Aws(aws) => aws.spawn_cluster(cluster, nodes).await,
            CloudProvider::Vultr(vultr) => vultr.spawn_cluster(cluster, nodes).await,
        }
    }
}
