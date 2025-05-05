use crate::commands::utils::ProgressTracker;
use crate::database::models::{Cluster, ConfigVar, ConfigVarFinder, InstanceType, Node};
use crate::integrations::{CloudInterface, data_transfer_objects::MachineImageDetail};
use anyhow::{Error, Result, anyhow};
use reqwest::{Client as HttpClient, header};
use serde_json::Value;
use std::collections::HashMap;
use tracing::warn;

pub struct VultrInterface {
    pub config_vars: Vec<ConfigVar>,
}

impl VultrInterface {
    const API_BASE_URL: &'static str = "https://api.vultr.com/v2";
    pub fn get_http_client(&self) -> Result<HttpClient, Error> {
        let api_key = match self.config_vars.get_value("API_KEY") {
            Some(api_key) => api_key.to_string(),
            None => {
                let error_msg = "Key 'API_KEY' not found in Vultr config_vars";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };
        let mut headers = header::HeaderMap::new();

        let auth_header = match header::HeaderValue::from_str(&format!("Bearer {}", api_key)) {
            Ok(header) => header,
            Err(err) => {
                return self
                    .handle_error(err.into(), "Failed to create Vultr authorization header");
            }
        };

        headers.insert("Authorization", auth_header);

        match HttpClient::builder().default_headers(headers).build() {
            Ok(client) => Ok(client),
            Err(err) => self.handle_error(err.into(), "Failed building Vultr HTTP client"),
        }
    }

    // Helper function to make API requests and parse JSON response
    async fn make_api_request(&self, endpoint: &str) -> Result<Value, Error> {
        let client = self.get_http_client()?;
        let url = format!("{}{}", Self::API_BASE_URL, endpoint);

        let response = match client.get(&url).send().await {
            Ok(result) => result,
            Err(err) => {
                return self.handle_error(err.into(), "Failed fetching Vultr API");
            }
        };

        let response_status = response.status();
        let json_response = match response_status.is_success() {
            true => match response.json().await {
                Ok(result) => result,
                Err(err) => {
                    return self.handle_error(err.into(), "Failed to parse Vultr API response");
                }
            },
            false => {
                let body = match response.text().await {
                    Ok(body) => body,
                    Err(err) => {
                        return self.handle_error(err.into(), "Unable to read HTTP response body");
                    }
                };
                let error_msg = format!(
                    "Vultr API returned error status {}: {}",
                    response_status, body
                );
                return self.handle_error(anyhow!("{}", error_msg), &error_msg);
            }
        };

        Ok(json_response)
    }
}

impl CloudInterface for VultrInterface {
    async fn fetch_regions(&self, _tracker: &ProgressTracker) -> Result<Vec<String>, Error> {
        let json_response = self.make_api_request("/regions").await?;

        let regions = match json_response["regions"].as_array() {
            Some(regions) => regions,
            None => {
                let error_msg = "Missing 'regions' array from Vultr API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        let region_ids = regions
            .iter()
            .filter_map(|region| region["id"].as_str().map(String::from))
            .collect();

        Ok(region_ids)
    }

    async fn fetch_zones(
        &self,
        region: &str,
        tracker: &ProgressTracker,
    ) -> Result<Vec<String>, Error> {
        // Vultr doesn't have a direct "zones" concept like some other cloud providers.
        // In Vultr, the closest equivalent would be availability within a region.
        // We'll return the region itself as the only zone for now.
        let regions = self.fetch_regions(tracker).await?;
        if !regions.contains(&region.to_string()) {
            let error_msg = format!("Invalid Vultr region: {}", region);
            return self.handle_error(anyhow!("{}", error_msg), &error_msg);
        }

        Ok(vec![region.to_string()])
    }

    async fn fetch_instance_types(
        &self,
        region: &str,
        _tracker: &ProgressTracker,
    ) -> Result<Vec<InstanceType>, Error> {
        let mut instance_types: Vec<InstanceType> = vec![];
        let locations: Vec<serde_json::Value> = Vec::new();

        // Common Vultr instance information
        let cpu_architecture = "x86_64".to_string(); // Vultr offers only x86 machines
        let core_count = None; // Vultr gives no info on 'cores'
        let threads_per_core = None; // Vultr gives no info on 'threads_per_core'
        let fpga_count = 0; // Vultr offers no FPGAs
        let fpga_type = None; // Vultr offers no FPGAs
        let supports_spot = false; // Vultr offers no spot instances
        let is_burstable = false; // Vultr offers no burstable instances
        let supports_efa = false; // Vultr offers no EFA support
        let has_affinity_settings = false; // Vultr offers no affinity settings
        let spot_price_per_hour = None; // Vultr offers no spot instances
        let provider_id = "vultr".to_string();

        // Fetch Virtualized plans information from Vultr
        let json_response = self.make_api_request("/plans?per_page=500").await?;

        let plans = match json_response["plans"].as_array() {
            Some(plans) => plans,
            None => {
                let error_msg = "Missing 'plans' array from Vultr API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        for plan in plans {
            let locations = plan["locations"]
                .as_array()
                .unwrap_or(&locations)
                .iter()
                .filter_map(|loc| loc.as_str())
                .collect::<Vec<_>>();

            if !locations.contains(&region) {
                continue; // Skip plans not available in this region
            }

            let name = match plan["id"].as_str() {
                Some(id_str) => id_str.to_string(),
                None => {
                    warn!("Skipping unknown instance_type: failed fetching id information.");
                    continue;
                }
            };
            let vcpus = match plan.get("vcpu_count").and_then(|v| v.as_i64()) {
                Some(count) => count,
                None => {
                    warn!(
                        "Skipping instance_type '{}': failed fetching vcpus information.",
                        name
                    );
                    continue;
                }
            };
            let name_lowercase = name.to_lowercase();
            let cpu_type = if name_lowercase.contains("amd") {
                "AMD".to_string()
            } else if name_lowercase.contains("intel") {
                "Intel".to_string()
            } else {
                "Unspecified".to_string()
            };
            let (gpu_type, gpu_count) = match plan["gpu_type"].as_str() {
                Some(gpu_type) => (Some(gpu_type.to_string()), 1),
                None => (None, 0),
            };
            let memory_in_mb = match plan.get("ram").and_then(|v| v.as_i64()) {
                Some(ram) => ram,
                None => {
                    warn!(
                        "Skipping instance_type '{}': failed fetching memory information.",
                        name
                    );
                    continue;
                }
            };
            let memory_in_mib =
                (memory_in_mb as f64 * 1000.0 * 1000.0 / (1024.0 * 1024.0)).round() as i64;
            let is_baremetal = false;
            let on_demand_price_per_hour: Option<f64> = plan["hourly_cost"].as_f64();

            let instance_type = InstanceType {
                name: name.clone(),
                cpu_architecture: cpu_architecture.clone(),
                vcpus,
                core_count,
                threads_per_core,
                cpu_type,
                gpu_count,
                gpu_type,
                fpga_count,
                fpga_type: fpga_type.clone(),
                memory_in_mib,
                supports_spot,
                is_baremetal,
                is_burstable,
                supports_efa,
                has_affinity_settings,
                on_demand_price_per_hour,
                spot_price_per_hour,
                region: region.to_string(),
                provider_id: provider_id.clone(),
            };

            instance_types.push(instance_type);
        }

        // Fetch Baremetal plan information from Vultr
        let json_response = self.make_api_request("/plans-metal?per_page=500").await?;
        let plans = match json_response["plans_metal"].as_array() {
            Some(plans) => plans,
            None => {
                let error_msg = "Missing 'plans_metal' array from Vultr API response";
                return self.handle_error(anyhow!(error_msg), error_msg);
            }
        };

        for plan in plans {
            let locations = plan["locations"]
                .as_array()
                .unwrap_or(&locations)
                .iter()
                .filter_map(|loc| loc.as_str())
                .collect::<Vec<_>>();

            if !locations.contains(&region) {
                continue; // Skip plans not available in this region
            }

            let name = match plan["id"].as_str() {
                Some(id_str) => id_str.to_string(),
                None => {
                    warn!("Skipping unknown instance_type: failed fetching id information.");
                    continue;
                }
            };
            let vcpus = match plan.get("cpu_threads").and_then(|v| v.as_i64()) {
                Some(count) => count,
                None => {
                    warn!(
                        "Skipping instance_type '{}': failed fetching vcpus information.",
                        name
                    );
                    continue;
                }
            };
            let core_count = plan["cpu_cores"].as_i64();
            let cpu_model = plan["cpu_model"].as_str();
            let cpu_manufacturer = plan["cpu_manufacturer"].as_str();
            let cpu_type_raw = match (cpu_manufacturer, cpu_model) {
                (Some(manufacturer), Some(model)) => format!("{} {}", manufacturer, model),
                (Some(manufacturer), None) => manufacturer.to_string(),
                (None, Some(model)) => model.to_string(),
                (None, None) => "Unspecified".to_string(),
            };
            let cpu_type = cpu_type_raw.replace("\"", "");
            let (gpu_type, gpu_count) = match plan["gpu_type"].as_str() {
                Some(gpu_type) => (Some(gpu_type.to_string()), 1),
                None => (None, 0),
            };
            let memory_in_mb = match plan.get("ram").and_then(|v| v.as_i64()) {
                Some(ram) => ram,
                None => {
                    warn!(
                        "Skipping instance_type '{}': failed fetching memory information.",
                        name
                    );
                    continue;
                }
            };
            let memory_in_mib =
                (memory_in_mb as f64 * 1000.0 * 1000.0 / (1024.0 * 1024.0)).round() as i64;
            let is_baremetal = true;
            let on_demand_price_per_hour: Option<f64> = plan["hourly_cost"].as_f64();

            let instance_type = InstanceType {
                name: name.clone(),
                cpu_architecture: cpu_architecture.clone(),
                vcpus,
                core_count,
                threads_per_core,
                cpu_type,
                gpu_count,
                gpu_type,
                fpga_count,
                fpga_type: fpga_type.clone(),
                memory_in_mib,
                supports_spot,
                is_baremetal,
                is_burstable,
                supports_efa,
                has_affinity_settings,
                on_demand_price_per_hour,
                spot_price_per_hour,
                region: region.to_string(),
                provider_id: provider_id.clone(),
            };

            instance_types.push(instance_type);
        }

        Ok(instance_types)
    }

    async fn fetch_prices(
        &self,
        _region: &str,
        _instance_types: &[String],
        _tracker: &ProgressTracker,
    ) -> Result<HashMap<String, f64>, Error> {
        // Vultr does not need a separate fetch prices method, as the pricing info
        // is returned in the instance_types api calls already.
        let map: HashMap<String, f64> = HashMap::new();

        Ok(map)
    }

    async fn fetch_machine_image(
        &self,
        _region: &str,
        _image_id: &str,
    ) -> Result<MachineImageDetail, Error> {
        anyhow::bail!("TODO")
    }

    async fn spawn_cluster(&self, _cluster: Cluster, _nodes: Vec<Node>) -> Result<(), Error> {
        anyhow::bail!("TODO")
    }
}
