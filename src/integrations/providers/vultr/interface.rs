use crate::database::models::{ConfigVar, ConfigVarFinder};

use anyhow::{Result, bail};
use reqwest::{Client as HttpClient, header};
use serde_json::Value as JsonValue;
use tracing::error;

pub struct VultrInterface {
    pub config_vars: Vec<ConfigVar>,
}

impl VultrInterface {
    pub const API_BASE_URL: &'static str = "https://api.vultr.com/v2";

    pub fn get_http_client(&self) -> Result<HttpClient> {
        let api_key = match self.config_vars.get_value("API_KEY") {
            Some(value) => value.to_string(),
            None => {
                bail!("Key 'API_KEY' not found in Vultr config_vars")
            }
        };

        let mut headers = header::HeaderMap::new();
        let auth_header = match header::HeaderValue::from_str(&format!("Bearer {}", api_key)) {
            Ok(header) => header,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed to create Vultr authorization header")
            }
        };

        headers.insert("Authorization", auth_header);

        match HttpClient::builder().default_headers(headers).build() {
            Ok(client) => Ok(client),
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed building Vultr HTTP client")
            }
        }
    }

    // Helper function to make API requests and parse JSON response
    pub async fn make_api_request(&self, endpoint: &str) -> Result<JsonValue> {
        let client = self.get_http_client()?;
        let url = format!("{}{}", Self::API_BASE_URL, endpoint);

        let response = match client.get(&url).send().await {
            Ok(result) => result,
            Err(e) => {
                error!("{:?}", e);
                bail!("Failed fetching Vultr API")
            }
        };

        let response_status = response.status();
        let json_response = match response_status.is_success() {
            true => match response.json().await {
                Ok(result) => result,
                Err(e) => {
                    error!("{:?}", e);
                    bail!("Failed to parse Vultr API response")
                }
            },
            false => {
                let body = match response.text().await {
                    Ok(body) => body,
                    Err(e) => {
                        error!("{:?}", e);
                        bail!("Unable to read HTTP response body")
                    }
                };
                error!(
                    "Vultr API returned error status {}: {}",
                    response_status, body
                );
                bail!("Vultr API returned an error")
            }
        };

        Ok(json_response)
    }
}
