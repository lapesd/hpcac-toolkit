use crate::database::models::{InstanceType, MachineImage};
use crate::integrations::CloudInfoProvider;
use crate::utils::ProgressTracker;

use anyhow::Result;
use std::collections::HashMap;

use super::interface::AwsInterface;

pub struct EbsPricing {
    pub storage_per_gb_month: f64,
    pub throughput_per_mbs_month: f64,
}

impl CloudInfoProvider for AwsInterface {
    async fn fetch_regions(&self, _tracker: &ProgressTracker) -> Result<Vec<String>> {
        // Use a default region (here "us-east-1") to create the client,
        // as the describe_regions API call is global.
        let client = self.get_ec2_client("us-east-1").await?;

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
                tracing::error!("{:?}", e);
                anyhow::bail!("Failed to fetch AWS regions")
            }
        }
    }

    async fn fetch_zones(
        &self,
        region: &str,
        _tracker: &ProgressTracker,
    ) -> Result<Vec<String>, anyhow::Error> {
        let client = self.get_ec2_client(region).await?;
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
                tracing::error!("{:?}", e);
                anyhow::bail!(
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
        let ec2_client = self.get_ec2_client(region).await?;
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
                    tracing::error!("{:?}", e);
                    anyhow::bail!("Failed to fetch AWS instance types for region '{}'", region)
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
                                tracing::warn!(
                                    "Skipping instance '{}': missing vCPU information",
                                    name
                                );
                                continue;
                            }

                            (
                                vcpus,
                                info.default_cores.map(|c| c.into()),
                                info.default_threads_per_core.map(|tpc| tpc.into()),
                            )
                        }
                        None => {
                            tracing::warn!(
                                "Skipping instance '{}': missing vCPU information",
                                name
                            );
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
                                tracing::warn!(
                                    "Skipping instance '{}': missing memory information",
                                    name
                                );
                                continue;
                            }
                            memory
                        }
                        None => {
                            tracing::warn!(
                                "Skipping instance '{}': missing memory information",
                                name
                            );
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
        let client = self.get_pricing_client().await?;

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
                            tracker.inc(1);
                            continue;
                        }
                    },
                    None => {
                        tracker.progress_bar.println(format!(
                            "Pricing data for instance_type '{}' not found",
                            it_name
                        ));
                        tracker.inc(1);
                        continue;
                    }
                },
                None => {
                    tracker.progress_bar.println(format!(
                        "No pricing data returned for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc(1);
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
                        tracker.inc(1);
                        continue;
                    }
                }
                None => {
                    tracker.progress_bar.println(format!(
                        "Product attributes not found for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc(1);
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
                                                tracker.inc(1);
                                            }
                                            Err(_) => {
                                                tracker.progress_bar.println(format!(
                                                    "Failed to parse price for instance_type: '{}'",
                                                    it_name
                                                ));
                                                tracker.inc(1);
                                            }
                                        },
                                        None => {
                                            tracker.progress_bar.println(format!(
                                                "USD price not found for instance_type: '{}'",
                                                it_name
                                            ));
                                            tracker.inc(1);
                                        }
                                    }
                                }
                                None => {
                                    tracker.progress_bar.println(format!(
                                        "No price dimension found for instance_type: '{}'",
                                        it_name
                                    ));
                                    tracker.inc(1);
                                }
                            },
                            None => {
                                tracker.progress_bar.println(format!(
                                    "priceDimensions object missing for instance_type: '{}'",
                                    it_name
                                ));
                                tracker.inc(1);
                            }
                        }
                    }
                    None => {
                        tracker.progress_bar.println(format!(
                            "No on-demand offer found for instance_type: '{}'",
                            it_name
                        ));
                        tracker.inc(1);
                    }
                },
                None => {
                    tracker.progress_bar.println(format!(
                        "On-demand price dimensions not found for instance_type: '{}'",
                        it_name
                    ));
                    tracker.inc(1);
                    continue;
                }
            };
        }

        Ok(price_map)
    }

    async fn fetch_machine_image(&self, region: &str, image_id: &str) -> Result<MachineImage> {
        let client = self.get_ec2_client(region).await?;
        let response = match client.describe_images().image_ids(image_id).send().await {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("{:?}", e);
                anyhow::bail!(
                    "Failed to fetch image '{}' in region '{}'",
                    image_id,
                    region
                )
            }
        };

        let images = response.images();

        let aws_image = match images.first() {
            Some(image) => image,
            None => {
                anyhow::bail!(
                    "Image '{}' not found in region '{}'. The image may not exist, may not be accessible, or may be in a different region.",
                    image_id,
                    region
                );
            }
        };

        let now = chrono::Utc::now().naive_utc();
        let image = MachineImage {
            id: image_id.to_string(),
            name: aws_image.name().unwrap_or_default().to_string(),
            description: aws_image.description().unwrap_or_default().to_string(),
            owner: aws_image.owner_id().unwrap_or_default().to_string(),
            creation_date: aws_image.creation_date().unwrap_or_default().to_string(),
            provider: "aws".to_string(),
            region: region.to_string(),
            created_at: now,
            updated_at: now,
        };

        Ok(image)
    }
}

impl AwsInterface {
    /// Fetch EBS pricing for a given volume type and region from the AWS Pricing API.
    /// Returns storage cost per GB-month and (for gp3) provisioned throughput cost per MB/s-month.
    pub async fn fetch_ebs_pricing(&self, region: &str, volume_type: &str) -> Result<EbsPricing> {
        let client = self.get_pricing_client().await?;

        let make_filter = |field: &str, value: &str| {
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field(field)
                .value(value)
                .build()
        };

        // Helper: extract USD on-demand price from a pricing JSON blob
        let extract_price = |json: &serde_json::Value| -> Option<f64> {
            json["terms"]["OnDemand"]
                .as_object()?
                .values()
                .next()?
                .get("priceDimensions")?
                .as_object()?
                .values()
                .next()?
                .get("pricePerUnit")?
                .get("USD")?
                .as_str()?
                .parse::<f64>()
                .ok()
        };

        // Fetch storage price ($/GB-month)
        let storage_response = client
            .get_products()
            .service_code("AmazonEC2")
            .format_version("aws_v1")
            .filters(make_filter("productFamily", "Storage")?)
            .filters(make_filter("volumeApiName", volume_type)?)
            .filters(make_filter("regionCode", region)?)
            .send()
            .await?;

        let storage_per_gb_month = storage_response
            .price_list()
            .iter()
            .find_map(|item| {
                let json: serde_json::Value = serde_json::from_str(item).ok()?;
                extract_price(&json)
            })
            .unwrap_or_else(|| {
                tracing::warn!("EBS storage price not found for {} in {}, using 0.08", volume_type, region);
                0.08
            });

        // Fetch provisioned throughput price ($/MB/s-month) — only relevant for gp3
        let throughput_per_mbs_month = if volume_type == "gp3" {
            let throughput_response = client
                .get_products()
                .service_code("AmazonEC2")
                .format_version("aws_v1")
                .filters(make_filter("productFamily", "Provisioned Throughput")?)
                .filters(make_filter("group", "EBS Throughput")?)
                .filters(make_filter("volumeApiName", volume_type)?)
                .filters(make_filter("regionCode", region)?)
                .send()
                .await?;

            let price_list = throughput_response.price_list();
            // AWS Pricing API returns gp3 throughput as $/GBps-month; divide by 1000 to get $/MBps-month.
            let gbps_per_month = price_list
                .iter()
                .find_map(|item| {
                    let json: serde_json::Value = serde_json::from_str(item).ok()?;
                    extract_price(&json)
                })
                .unwrap_or_else(|| {
                    tracing::warn!("EBS throughput price not found for gp3 in {}, using 40.96", region);
                    40.96
                });
            gbps_per_month / 1000.0
        } else {
            0.0
        };

        Ok(EbsPricing {
            storage_per_gb_month,
            throughput_per_mbs_month,
        })
    }

    /// Fetch current spot prices for the given instance types via EC2 describe_spot_price_history.
    /// Returns a map of instance_type -> current spot price (USD/hour).
    /// If availability_zone is empty, prices from any AZ in the region are accepted.
    pub async fn fetch_spot_prices(
        &self,
        region: &str,
        instance_type_names: &[String],
        availability_zone: &str,
    ) -> Result<HashMap<String, f64>> {
        let client = self.get_ec2_client(region).await?;

        let instance_types: Vec<aws_sdk_ec2::types::InstanceType> = instance_type_names
            .iter()
            .map(|s| aws_sdk_ec2::types::InstanceType::from(s.as_str()))
            .collect();

        let mut request = client
            .describe_spot_price_history()
            .set_instance_types(Some(instance_types))
            .product_descriptions("Linux/UNIX")
            .start_time(aws_sdk_ec2::primitives::DateTime::from_secs(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            ));

        if !availability_zone.is_empty() {
            request = request.availability_zone(availability_zone);
        }

        let response = request.send().await?;

        let mut price_map: HashMap<String, f64> = HashMap::new();
        for entry in response.spot_price_history() {
            let it = match entry.instance_type() {
                Some(t) => t.as_str().to_string(),
                None => continue,
            };
            if price_map.contains_key(&it) {
                continue; // already have the most recent entry for this type
            }
            if let Some(price_str) = entry.spot_price() {
                if let Ok(price) = price_str.parse::<f64>() {
                    price_map.insert(it, price);
                }
            }
        }

        Ok(price_map)
    }
}
