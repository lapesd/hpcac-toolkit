use crate::database::models::{InstanceType, MachineImage};
use crate::integrations::CloudInfoProvider;
use crate::utils::ProgressTracker;

use anyhow::{Result, bail};
use std::collections::HashMap;

use super::interface::GcpInterface;

impl CloudInfoProvider for GcpInterface {
    async fn fetch_regions(&self, _tracker: &ProgressTracker) -> Result<Vec<String>> {
        bail!("Not implemented")
    }

    async fn fetch_zones(&self, _region: &str, _tracker: &ProgressTracker) -> Result<Vec<String>> {
        bail!("Not implemented")
    }

    async fn fetch_instance_types(
        &self,
        _region: &str,
        _tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>> {
        bail!("Not implemented")
    }

    async fn fetch_prices(
        &self,
        _region: &str,
        _instance_types: &[String],
        _tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>> {
        bail!("Not implemented")
    }

    async fn fetch_machine_image(&self, _region: &str, _image_id: &str) -> Result<MachineImage> {
        bail!("Not implemented")
    }
}
