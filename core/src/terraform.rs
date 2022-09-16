use hcl::{block, Block};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct TerraformPlugin {
    alias: String,
    source: String,
    version: String,
}

pub struct TerraformProvidersOptions {
    pub provider_plugin: TerraformPlugin,
}

impl TerraformProvidersOptions {
    pub fn default() -> Self {
        Self {
            provider_plugin: TerraformPlugin {
                alias: "aws".to_string(),
                source: "hashicorp/aws".to_string(),
                version: "~> 4.30.0".to_string(),
            },
        }
    }

    fn to_hcl(&self) -> Block {
        block!(
            terraform {
                required_providers {
                    (&self.provider_plugin.alias) = {
                        source = (&self.provider_plugin.source)
                        version = (&self.provider_plugin.version)
                    }
                    null = {
                        source = "hashicorp/null"
                    }
                    tls = {
                        source = "hashicorp/tls"
                    }
                }
            }
        )
    }

    pub fn save_hcl(self, terraform_dir: &str) -> std::io::Result<()> {
        let path_str = format!("{}/versions.tf", terraform_dir);
        let path = Path::new(&path_str);
        let hcl_block = self.to_hcl();
        let hcl_string = hcl::to_string(&hcl_block).unwrap();
        let mut file = File::create(path)?;
        file.write_all(hcl_string.as_bytes())?;
        Ok(())
    }
}

#[test]
fn test_terraform_versions_hcl_generation() {
    let tfversions = TerraformProvidersOptions::default();
    let expected = r#"
terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
      version = "~> 4.30.0"
    }
    null = {
      source = "hashicorp/null"
    }
    tls = {
      source = "hashicorp/tls"
    }
  }
}
"#
    .trim_start();

    let hcl = hcl::to_string(&tfversions.to_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
