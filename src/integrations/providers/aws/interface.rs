use crate::database::models::{Cluster, ConfigVar, ConfigVarFinder};

use anyhow::{Result, bail};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_ec2::Client as EC2Client;
use aws_sdk_pricing::Client as PricingClient;
use aws_sdk_servicequotas::Client as ServiceQuotasClient;

/// Context struct containing all cluster-related information and resource identifiers
/// used throughout the cluster lifecycle operations
pub struct AwsClusterContext {
    // AWS SDK Client
    pub client: EC2Client,

    // Core cluster information for tagging and filtering
    pub cluster_id: String,
    pub cluster_id_tag: aws_sdk_ec2::types::Tag,
    pub cluster_id_filter: aws_sdk_ec2::types::Filter,

    // Standardized resource naming
    pub vpc_name: String,
    pub subnet_name: String,
    pub gateway_name: String,
    pub route_table_name: String,
    pub security_group_name: String,
    pub placement_group_name: String,
    pub ssh_key_name: String,

    // Resource identifiers (populated during creation/discovery)
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub gateway_id: Option<String>,
    pub route_table_id: Option<String>,
    pub security_group_ids: Vec<String>,
    pub placement_group_name_actual: Option<String>,
    pub ssh_key_id: Option<String>,

    // Cluster and network configuration
    pub availability_zone: String,
    pub use_node_affinity: bool,
    pub use_elastic_fabric_adapters: bool,
    pub public_ssh_key_path: String,
    pub vpc_cidr_block: String,
    pub subnet_cidr_block: String,
}

impl AwsClusterContext {
    /// Create a new ClusterContext from a Cluster and EC2Client
    pub fn new(cluster: &Cluster, client: EC2Client) -> Self {
        let cluster_id = cluster.id.to_string();
        let cluster_id_tag = aws_sdk_ec2::types::Tag::builder()
            .key("ClusterId")
            .value(&cluster_id)
            .build();
        let cluster_id_filter = aws_sdk_ec2::types::Filter::builder()
            .name("tag:ClusterId")
            .values(&cluster_id)
            .build();

        Self {
            cluster_id: cluster_id.clone(),
            cluster_id_tag,
            cluster_id_filter,
            client,

            // Generate resource names
            vpc_name: format!("{}-VPC", cluster_id),
            subnet_name: format!("{}-SUBNET", cluster_id),
            gateway_name: format!("{}-IGW", cluster_id),
            route_table_name: format!("{}-RT", cluster_id),
            security_group_name: format!("{}-SG", cluster_id),
            placement_group_name: format!("{}-PG", cluster_id),
            ssh_key_name: format!("{}-KEY", cluster_id),

            // Initialize resource IDs as None/empty
            vpc_id: None,
            subnet_id: None,
            gateway_id: None,
            route_table_id: None,
            security_group_ids: Vec::new(),
            placement_group_name_actual: None,
            ssh_key_id: None,

            // Copy some cluster configuration for convenience
            availability_zone: cluster.availability_zone.clone(),
            use_node_affinity: cluster.use_node_affinity,
            use_elastic_fabric_adapters: cluster.use_elastic_fabric_adapters,
            public_ssh_key_path: cluster.public_ssh_key_path.clone(),
            // TODO: Evaluate if it's desired to make the CIDR blocks configurable
            vpc_cidr_block: "10.0.0.0/16".to_string(),
            subnet_cidr_block: "10.0.1.0/24".to_string(),
        }
    }

    /// Generate a ec2 instance name for a specific node index
    pub fn ec2_instance_name(&self, node_index: usize) -> String {
        format!("{}-EC2-INSTANCE-{}", self.cluster_id, node_index)
    }

    /// Generate a network interface name for a specific node index
    pub fn network_interface_name(&self, node_index: usize) -> String {
        format!("{}-ENI-{}", self.cluster_id, node_index)
    }

    /// Generate an elastic ip name for a specific node index
    pub fn elastic_ip_name(&self, node_index: usize) -> String {
        format!("{}-EIP-{}", self.cluster_id, node_index)
    }

    /// Generate private IP for a specific node index
    pub fn network_interface_private_ip(&self, node_index: usize) -> String {
        format!("10.0.1.{}", node_index + 10)
    }
}

pub struct AwsInterface {
    pub config_vars: Vec<ConfigVar>,
}

impl AwsInterface {
    /// Build an AWS SDK configuration from ConfigVars
    pub fn get_config(&self, region: &str) -> Result<SdkConfig> {
        let access_key_id = match self.config_vars.get_value("ACCESS_KEY_ID") {
            Some(value) => value.to_string(),
            None => {
                bail!("Key 'ACCESS_KEY_ID' not found in config_vars")
            }
        };
        let secret_access_key = match self.config_vars.get_value("SECRET_ACCESS_KEY") {
            Some(value) => value.to_string(),
            None => {
                bail!("Key 'SECRET_ACCESS_KEY' not found in config_vars")
            }
        };
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
    pub fn get_ec2_client(&self, region: &str) -> Result<EC2Client> {
        let config = self.get_config(region)?;
        Ok(EC2Client::new(&config))
    }

    /// Get a Pricing client configured with the provided credentials and region.
    pub fn get_pricing_client(&self) -> Result<PricingClient> {
        let config = self.get_config("us-east-1")?;
        Ok(PricingClient::new(&config))
    }

    /// Get an Service Quotas client configured with the provided credentials and region.
    pub fn _get_service_quotas_client(&self, region: &str) -> Result<ServiceQuotasClient> {
        let config = self.get_config(region)?;
        Ok(ServiceQuotasClient::new(&config))
    }

    /// Create a ClusterContext for the given cluster
    pub fn create_cluster_context(&self, cluster: &Cluster) -> Result<AwsClusterContext> {
        let client = self.get_ec2_client(&cluster.region)?;
        Ok(AwsClusterContext::new(cluster, client))
    }
}
