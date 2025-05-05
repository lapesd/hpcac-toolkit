use crate::commands::utils::ProgressTracker;
use crate::database::models::{Cluster, ConfigVar, ConfigVarFinder, InstanceType, Node};
use crate::integrations::{CloudInterface, data_transfer_objects::MachineImageDetail};

use anyhow::{Error, Result, anyhow};
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::{Credentials, provider::SharedCredentialsProvider};
use aws_sdk_ec2::Client as EC2Client;
use aws_sdk_pricing::Client as PricingClient;
use aws_sdk_servicequotas::Client as ServiceQuotasClient;
use std::collections::HashMap;
use tracing::warn;

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

impl CloudInterface for AwsInterface {
    async fn fetch_regions(&self, _tracker: &ProgressTracker) -> Result<Vec<String>, Error> {
        // Use a default region (here "us-east-1") to create the client,
        // as the describe_regions API call is global.
        let client = match self.get_ec2_client("us-east-1") {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize AWS EC2 client"),
        };

        match client.describe_regions().send().await {
            Ok(resp) => {
                let regions: Vec<String> = resp
                    .regions
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| r.region_name)
                    .collect();

                Ok(regions)
            }
            Err(err) => self.handle_error(err.into(), "Failed to fetch AWS regions"),
        }
    }

    async fn fetch_zones(
        &self,
        region: &str,
        _tracker: &ProgressTracker,
    ) -> Result<Vec<String>, anyhow::Error> {
        let client = match self.get_ec2_client(region) {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize AWS EC2 client"),
        };
        match client.describe_availability_zones().send().await {
            Ok(resp) => {
                let zones: Vec<String> = resp
                    .availability_zones
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| r.zone_name)
                    .collect();

                Ok(zones)
            }
            Err(err) => self.handle_error(
                err.into(),
                &format!(
                    "Failed to fetch AWS availability zones for region '{}'",
                    region
                ),
            ),
        }
    }

    async fn fetch_instance_types(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>, Error> {
        let ec2_client = match self.get_ec2_client(region) {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize AWS EC2 client"),
        };
        let mut instance_types: Vec<InstanceType> = vec![];
        let mut next_token: Option<String> = None;
        let base_request = ec2_client.describe_instance_types();

        loop {
            let mut request = base_request.clone();
            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            let resp = match request.send().await {
                Ok(resp) => resp,
                Err(err) => {
                    return self.handle_error(
                        err.into(),
                        &format!("Failed to fetch AWS instance types for region '{}'", region),
                    );
                }
            };

            let instance_types_batch = resp.instance_types.unwrap_or_default().into_iter();
            for item in instance_types_batch {
                // Reference: https://docs.rs/aws-sdk-ec2/latest/aws_sdk_ec2/client/struct.Client.html#impl-Client-286
                if let Some(aws_it) = item.instance_type {
                    let name = aws_it.as_str().to_string();

                    // Extract CPU information
                    let (vcpus, core_count, threads_per_core) = match item.v_cpu_info.as_ref() {
                        Some(info) => {
                            let vcpus: i64 = info.default_v_cpus.unwrap_or(0).into();
                            if vcpus == 0 {
                                warn!("Skipping instance '{}': missing vCPU information", name);
                                continue;
                            }

                            (
                                vcpus,
                                info.default_cores.map(|c| c.into()),
                                info.default_threads_per_core.map(|tpc| tpc.into()),
                            )
                        }
                        None => {
                            warn!("Skipping instance '{}': missing vCPU information", name);
                            continue;
                        }
                    };

                    // Extract processor data
                    let (cpu_type, cpu_architecture) =
                        if let Some(processor_info) = &item.processor_info {
                            let cpu_type = processor_info.manufacturer.clone().unwrap_or_default();
                            let cpu_architecture = processor_info
                                .supported_architectures
                                .as_ref()
                                .map(|archs| {
                                    archs
                                        .iter()
                                        .map(|arch| arch.to_string())
                                        .collect::<Vec<_>>()
                                        .join(",")
                                })
                                .unwrap_or_default();
                            (cpu_type, cpu_architecture)
                        } else {
                            (String::new(), String::new())
                        };

                    // Extract GPU information
                    let (gpu_count, gpu_type) = if let Some(gpu_info) = &item.gpu_info {
                        let mut gpu_count: i64 = 0;
                        let gpu_type = gpu_info
                            .gpus
                            .as_ref()
                            .and_then(|gpus| gpus.first())
                            .map(|gpu| {
                                gpu_count = gpu.count.unwrap_or(0).into();
                                Some(format!(
                                    "{} {}",
                                    gpu.manufacturer.as_deref().unwrap_or(""),
                                    gpu.name.as_deref().unwrap_or("")
                                ))
                            })
                            .unwrap_or_default();
                        (gpu_count, gpu_type)
                    } else {
                        (0, None)
                    };

                    // Extract FPGA information
                    let (fpga_count, fpga_type) = if let Some(fpga_info) = &item.fpga_info {
                        let mut fpga_count: i64 = 0;
                        let fpga_type = fpga_info
                            .fpgas
                            .as_ref()
                            .and_then(|fpgas| fpgas.first())
                            .map(|fpga| {
                                fpga_count = fpga.count.unwrap_or(0).into();
                                Some(format!(
                                    "{} {}",
                                    fpga.manufacturer.as_deref().unwrap_or(""),
                                    fpga.name.as_deref().unwrap_or("")
                                ))
                            })
                            .unwrap_or_default();
                        (fpga_count, fpga_type)
                    } else {
                        (0, None)
                    };

                    // Extract RAM information
                    let memory_in_mib = match item.memory_info.as_ref() {
                        Some(info) => {
                            let memory: i64 = info.size_in_mib.unwrap_or(0);
                            if memory == 0 {
                                warn!("Skipping instance '{}': missing memory information", name);
                                continue;
                            }
                            memory
                        }
                        None => {
                            warn!("Skipping instance '{}': missing memory information", name);
                            continue;
                        }
                    };

                    // Extract affinity settings
                    let has_affinity_settings = if let Some(placement_group_info) =
                        &item.placement_group_info
                    {
                        placement_group_info
                            .supported_strategies
                            .as_ref()
                            .map(|strategies| {
                                strategies
                                    .contains(&aws_sdk_ec2::types::PlacementGroupStrategy::Cluster)
                            })
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    let supports_spot = item
                        .supported_usage_classes
                        .as_ref()
                        .map(|info| info.contains(&aws_sdk_ec2::types::UsageClassType::Spot))
                        .unwrap_or(false);
                    let is_baremetal = item.bare_metal.unwrap_or(false);
                    let is_burstable = item.burstable_performance_supported.unwrap_or(false);
                    let supports_efa = item
                        .network_info
                        .as_ref()
                        .and_then(|info| info.efa_supported)
                        .unwrap_or(false);

                    let instance_type = InstanceType {
                        name: name.clone(),
                        cpu_architecture,
                        vcpus,
                        core_count,
                        threads_per_core,
                        cpu_type,
                        gpu_count,
                        gpu_type,
                        fpga_count,
                        fpga_type,
                        memory_in_mib,
                        supports_spot,
                        is_baremetal,
                        is_burstable,
                        supports_efa,
                        has_affinity_settings,
                        on_demand_price_per_hour: None, // Will be added later
                        spot_price_per_hour: None,      // Will be added later
                        region: region.to_string(),
                        provider_id: "aws".to_string(),
                    };

                    instance_types.push(instance_type);
                }
            }

            next_token = resp.next_token;
            if next_token.is_none() {
                break;
            }
        }

        // Fetch pricing information
        if !instance_types.is_empty() {
            let instance_type_names: Vec<String> =
                instance_types.iter().map(|it| it.name.clone()).collect();
            let price_map = self
                .fetch_prices(region, &instance_type_names, tracker)
                .await?;
            for instance_type in instance_types.iter_mut() {
                if let Some(price) = price_map.get(&instance_type.name) {
                    instance_type.on_demand_price_per_hour = Some(*price);
                }
            }
        }

        Ok(instance_types)
    }

    async fn fetch_prices(
        &self,
        region: &str,
        instance_type_names: &[String],
        tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>, Error> {
        let client = self.get_pricing_client()?;

        let mut price_map: HashMap<String, f64> = HashMap::new();
        let base_filters = vec![
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("ServiceCode")
                .value("AmazonEC2")
                .build()?,
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("regionCode")
                .value(region)
                .build()?,
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("operatingSystem")
                .value("Linux")
                .build()?,
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("preInstalledSw")
                .value("NA")
                .build()?,
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("tenancy")
                .value("Shared")
                .build()?,
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("capacitystatus")
                .value("Used")
                .build()?,
        ];

        let total = instance_type_names.len();
        for (i, it_name) in instance_type_names.iter().enumerate() {
            tracker.update_message(&format!(
                "Fetching price for '{}' ({}/{})",
                it_name,
                i + 1,
                total
            ));

            let mut filters = base_filters.clone();
            filters.push(
                aws_sdk_pricing::types::Filter::builder()
                    .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                    .field("instanceType")
                    .value(it_name)
                    .build()?,
            );

            let response = client
                .get_products()
                .service_code("AmazonEC2")
                .format_version("aws_v1")
                .set_filters(Some(filters))
                .max_results(1)
                .send()
                .await?;

            // Extract instance price data from provider response
            let price_item_json = match response.price_list {
                Some(price_list) => match price_list.first() {
                    Some(price_item) => match serde_json::from_str::<serde_json::Value>(price_item)
                    {
                        Ok(json_data) => json_data,
                        Err(e) => {
                            tracker.progress_bar.println(format!(
                                "Error parsing price data for {}: {}",
                                it_name, e
                            ));
                            tracker.inc();
                            continue;
                        }
                    },
                    None => {
                        tracker.progress_bar.println(format!(
                            "Pricing data for instance_type '{}' not found",
                            it_name
                        ));
                        tracker.inc();
                        continue;
                    }
                },
                None => {
                    tracker.progress_bar.println(format!(
                        "No pricing data returned for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc();
                    continue;
                }
            };

            // Double-check the instance_type from the response
            match price_item_json["product"]["attributes"]["instanceType"].as_str() {
                Some(response_name) => {
                    if response_name != it_name {
                        tracker.progress_bar.println(format!(
                            "Data mismatch found in pricing record for instance_type: '{}'",
                            it_name
                        ));
                        tracker.inc();
                        continue;
                    }
                }
                None => {
                    tracker.progress_bar.println(format!(
                        "Product attributes not found for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc();
                    continue;
                }
            };

            // Fetch pricing from the `OnDemand` data object
            match price_item_json["terms"]["OnDemand"].as_object() {
                Some(first_result) => match first_result.iter().next() {
                    Some((_, offer)) => {
                        match offer.get("priceDimensions").and_then(|v| v.as_object()) {
                            Some(price_dimensions) => match price_dimensions.iter().next() {
                                Some((_, dimension)) => {
                                    match dimension
                                        .get("pricePerUnit")
                                        .and_then(|v| v.get("USD"))
                                        .and_then(|v| v.as_str())
                                    {
                                        Some(price_str) => match price_str.parse::<f64>() {
                                            Ok(price) => {
                                                price_map.insert(it_name.clone(), price);
                                                tracker.inc();
                                            }
                                            Err(_) => {
                                                tracker.progress_bar.println(format!(
                                                    "Failed to parse price for instance_type: '{}'",
                                                    it_name
                                                ));
                                                tracker.inc();
                                            }
                                        },
                                        None => {
                                            tracker.progress_bar.println(format!(
                                                "USD price not found for instance_type: '{}'",
                                                it_name
                                            ));
                                            tracker.inc();
                                        }
                                    }
                                }
                                None => {
                                    tracker.progress_bar.println(format!(
                                        "No price dimension found for instance_type: '{}'",
                                        it_name
                                    ));
                                    tracker.inc();
                                }
                            },
                            None => {
                                tracker.progress_bar.println(format!(
                                    "priceDimensions object missing for instance_type: '{}'",
                                    it_name
                                ));
                                tracker.inc();
                            }
                        }
                    }
                    None => {
                        tracker.progress_bar.println(format!(
                            "No on-demand offer found for instance_type: '{}'",
                            it_name
                        ));
                        tracker.inc();
                    }
                },
                None => {
                    tracker.progress_bar.println(format!(
                        "On-demand price dimensions not found for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc();
                    continue;
                }
            };
        }

        Ok(price_map)
    }

    async fn fetch_machine_image(
        &self,
        region: &str,
        image_id: &str,
    ) -> Result<MachineImageDetail, Error> {
        let client = match self.get_ec2_client(region) {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize EC2 client"),
        };

        let resp = match client.describe_images().image_ids(image_id).send().await {
            Ok(resp) => resp,
            Err(err) => {
                return self.handle_error(
                    err.into(),
                    &format!(
                        "Failed to fetch image '{}' in region '{}'",
                        image_id, region
                    ),
                );
            }
        };

        let images = resp.images.unwrap_or_default();
        let aws_image = &images[0]; // get the first (should be only one)
        let image = MachineImageDetail {
            id: image_id.to_string(),
            name: aws_image.name.clone().unwrap_or_default(),
            description: aws_image.description.clone().unwrap_or_default(),
            owner: aws_image.owner_id.clone().unwrap_or_default(),
            creation_date: aws_image.creation_date.clone().unwrap_or_default(),
        };

        Ok(image)
    }

    async fn spawn_cluster(&self, cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        let client = match self.get_ec2_client(&cluster.region) {
            Ok(client) => client,
            Err(err) => return self.handle_error(err, "Failed to initialize EC2 client"),
        };

        let vpc_cidr_block = "10.0.0.6/16";
        let _create_vpc_request = client
            .create_vpc()
            .cidr_block(vpc_cidr_block)
            .instance_tenancy(aws_sdk_ec2::types::Tenancy::Default)
            .tag_specifications(
                aws_sdk_ec2::types::TagSpecification::builder()
                    .resource_type(aws_sdk_ec2::types::ResourceType::Vpc)
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("Name")
                            .value(format!("cluster-{}-vpc", cluster.id))
                            .build(),
                    )
                    .tags(
                        aws_sdk_ec2::types::Tag::builder()
                            .key("ClusterId")
                            .value(cluster.id.to_string())
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await;

        // Continue, after VPC, crete subnets, etc.. up to the nodes and security rules.

        Ok(())
    }
}
