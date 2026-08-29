use crate::database::models::{ConfigVar, ConfigVarFinder};

use anyhow::{Result, bail};
use reqwest::{Client as HttpClient, header};
use serde_json::Value as JsonValue;
use tracing::error;

pub struct GcpInterface {
    pub config_vars: Vec<ConfigVar>,
}

/// TODO: Decide over using the Rust SDK or the REST API...
/// Rust SDK: https://github.com/googleapis/google-cloud-rust/tree/main
/// REST API: https://cloud.google.com/compute/docs/authentication?hl=pt-br#rest
///
/// If using the API, check the Vultr code for an example on how to use the `reqwest` http
/// client for requests.
/// If using the Rust SDK, check the SDK own documentation and the AWS code to get an idea of
/// how to implement this method.
impl GcpInterface {}
