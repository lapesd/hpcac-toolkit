use crate::database::models::{ConfigVar, ConfigVarFinder};
use crate::integrations::CloudErrorHandler;

use anyhow::{Error, Result, anyhow};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_ec2::Client as EC2Client;
use aws_sdk_pricing::Client as PricingClient;
use aws_sdk_servicequotas::Client as ServiceQuotasClient;

pub struct AwsInterface {
    pub config_vars: Vec<ConfigVar>,
}

impl AwsInterface {
    /// Build an AWS SDK configuration from ConfigVars
    pub fn get_config(&self, region: &str) -> Result<SdkConfig> {
        let access_key_id = self
            .config_vars
            .get_value("ACCESS_KEY_ID")
            .ok_or_else(|| anyhow!("Key 'ACCESS_KEY_ID' not found in config_vars."))?
            .to_string();

        let secret_access_key = self
            .config_vars
            .get_value("SECRET_ACCESS_KEY")
            .ok_or_else(|| anyhow!("Key 'SECRET_ACCESS_KEY' not found in config_vars."))?
            .to_string();

        let credentials =
            Credentials::from_keys(access_key_id.clone(), secret_access_key.clone(), None);
        let static_provider = SharedCredentialsProvider::new(credentials);
        let region_struct = Region::new(region.to_string());

        let config = SdkConfig::builder()
            .behavior_version(BehaviorVersion::v2025_01_17())
            .region(region_struct)
            .credentials_provider(static_provider)
            .build();

        Ok(config)
    }

    /// Get an EC2 client configured with the provided credentials and region.
    pub fn get_ec2_client(&self, region: &str) -> Result<EC2Client, Error> {
        Ok(EC2Client::new(&self.get_config(region)?))
    }

    /// Get a Pricing client configured with the provided credentials and region.
    pub fn get_pricing_client(&self) -> Result<PricingClient, Error> {
        Ok(PricingClient::new(&self.get_config("us-east-1")?))
    }

    /// Get an Service Quotas client configured with the provided credentials and region.
    pub fn _get_service_quotas_client(&self, region: &str) -> Result<ServiceQuotasClient, Error> {
        Ok(ServiceQuotasClient::new(&self.get_config(region)?))
    }
}

impl CloudErrorHandler for AwsInterface {}
