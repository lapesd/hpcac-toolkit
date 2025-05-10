use crate::database::models::{ConfigVar, ConfigVarFinder};
use crate::integrations::CloudErrorHandler;
use anyhow::{Error, Result, anyhow};
use reqwest::{Client as HttpClient, header};
use serde_json::Value;

pub struct VultrInterface {
    pub config_vars: Vec<ConfigVar>,
}

impl VultrInterface {
    pub const API_BASE_URL: &'static str = "https://api.vultr.com/v2";

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
    pub async fn make_api_request(&self, endpoint: &str) -> Result<Value, Error> {
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

impl CloudErrorHandler for VultrInterface {}
