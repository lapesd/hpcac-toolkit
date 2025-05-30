use crate::database::models::{InstanceType, MachineImage};
use crate::integrations::CloudInfoProvider;
use crate::utils::ProgressTracker;

use anyhow::{Result, bail};
use std::collections::HashMap;
use tracing::{error, warn};

use super::interface::AwsInterface;

impl CloudInfoProvider for AwsInterface {
    async fn fetch_regions(&self, _tracker: &ProgressTracker) -> Result<Vec<String>> {
        // Use a default region (here "us-east-1") to create the client,
        // as the describe_regions API call is global.
        let client = self.get_ec2_client("us-east-1")?;

        match client.describe_regions().send().await {
            Ok(response) => {
                let regions: Vec<String> = response
                    .regions
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| r.region_name)
                    .collect();

                Ok(regions)
            }
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to fetch AWS regions")
            }
        }
    }

    async fn fetch_zones(
        &self,
        region: &str,
        _tracker: &ProgressTracker,
    ) -> Result<Vec<String>, anyhow::Error> {
        let client = self.get_ec2_client(region)?;
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
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to fetch AWS availability zones for region '{}'",
                    region
                )
            }
        }
    }

    async fn fetch_instance_types(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>> {
        let ec2_client = self.get_ec2_client(region)?;
        let mut instance_types: Vec<InstanceType> = vec![];
        let mut next_token: Option<String> = None;
        let base_request = ec2_client.describe_instance_types();

        loop {
            let mut request = base_request.clone();
            if let Some(token) = &next_token {
                request = request.next_token(token);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failed to fetch AWS instance types for region '{}'", region)
                }
            };

            let instance_types_batch = response.instance_types.unwrap_or_default().into_iter();
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
                        on_demand_price_per_hour: None,
                        spot_price_per_hour: None,
                        region: region.to_string(),
                        provider_id: "aws".to_string(),
                    };

                    instance_types.push(instance_type);
                }
            }

            next_token = response.next_token;
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
    ) -> Result<HashMap<String, f64>> {
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

    async fn fetch_machine_image(&self, region: &str, image_id: &str) -> Result<MachineImage> {
        let client = self.get_ec2_client(region)?;
        let response = match client.describe_images().image_ids(image_id).send().await {
            Ok(response) => response,
            Err(e) => {
                error!("{:?}", e);
                bail!(
                    "Failed to fetch image '{}' in region '{}'",
                    image_id,
                    region
                )
            }
        };

        let images = response.images.unwrap_or_default();
        let aws_image = &images[0]; // get the first (should be only one)
        let now = chrono::Utc::now().naive_utc();
        let image = MachineImage {
            id: image_id.to_string(),
            name: aws_image.name.clone().unwrap_or_default(),
            description: aws_image.description.clone().unwrap_or_default(),
            owner: aws_image.owner_id.clone().unwrap_or_default(),
            creation_date: aws_image.creation_date.clone().unwrap_or_default(),
            provider: "aws".to_string(),
            region: region.to_string(),
            created_at: now,
            updated_at: now,
        };

        Ok(image)
    }
}
