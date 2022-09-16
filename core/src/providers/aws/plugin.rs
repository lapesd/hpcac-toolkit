use hcl::{block, Block};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use super::variables::AwsRegion;

#[derive(Debug, Clone)]
pub struct AwsPluginOptions {
    alias: String,
    profile: String,
    access_key: String,
    secret_key: String,
    max_retries: usize,
    pub region: AwsRegion,
}

impl AwsPluginOptions {
    pub fn default(access_key: &str, secret_key: &str) -> Self {
        Self {
            alias: "aws".to_string(),
            profile: "default".to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
            max_retries: 3,
            region: AwsRegion::NorthVirginia,
        }
    }

    fn generate_hcl(&self) -> Block {
        block!(
            provider (&self.alias) {
                region = (self.region.to_string())
                profile = (self.profile)
                access_key = (self.access_key)
                secret_key = (self.secret_key)
                max_retries = (self.max_retries)
            }
        )
    }

    pub fn save_hcl(&self, terraform_dir: &str) -> std::io::Result<()> {
        let path_str = format!("{}/provider.tf", terraform_dir);
        let path = Path::new(&path_str);
        let hcl_block = self.generate_hcl();
        let hcl_string = hcl::to_string(&hcl_block).unwrap();
        let mut file = File::create(path)?;
        file.write_all(hcl_string.as_bytes())?;
        Ok(())
    }
}

#[test]
fn test_aws_plugin_options_default_trait() {
    let access_key = "access_key";
    let secret_key = "secret_key";
    let provider_config = AwsPluginOptions::default(access_key, secret_key);
    assert_eq!(provider_config.region, AwsRegion::NorthVirginia);
    assert_eq!(provider_config.profile, "default".to_string());
}

#[test]
fn test_aws_plugin_options_hcl_generation() {
    let tfprovider = AwsPluginOptions::default("access_key", "secret_key");
    let expected = r#"
provider "aws" {
  region = "us-east-1"
  profile = "default"
  access_key = "access_key"
  secret_key = "secret_key"
  max_retries = 3
}
"#
    .trim_start();

    let hcl = hcl::to_string(&tfprovider.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
