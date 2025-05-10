use crate::database::models::{CloudResource, ResourceStatus};
use crate::integrations::CloudResourceTracker;

use anyhow::{Error, Result};

use super::interface::AwsInterface;

impl CloudResourceTracker for AwsInterface {
    async fn list_resources_by_cluster(
        &self,
        _cluster_id: &str,
    ) -> Result<Vec<CloudResource>, Error> {
        anyhow::bail!("Not implemented")
    }

    async fn get_resource_status(&self, _resource_id: &str) -> Result<ResourceStatus, Error> {
        anyhow::bail!("Not implemented")
    }
}
