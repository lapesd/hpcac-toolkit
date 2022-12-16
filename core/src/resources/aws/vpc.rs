use hcl;

use super::base::{AwsResource, AwsResourceType};


pub struct AwsVpc {
    resource_type: AwsResourceType,
    resource_name: String,
    cidr_block: String,
    enable_dns_support: bool,
    enable_dns_hostnames: bool,
}

impl Default for AwsVpc {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsVpc,
            resource_name: "cluster_vpc".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            enable_dns_support: true,
            enable_dns_hostnames: true,
        }
    }
}

impl AwsResource for AwsVpc {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))    
    }

    fn generate_hcl(&self) -> hcl::Body {
        let vpc_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("cidr_block", self.cidr_block.to_owned()))
            .add_attribute(("enable_dns_support", self.enable_dns_support))
            .add_attribute(("enable_dns_hostnames", self.enable_dns_hostnames))
            .build();

        hcl::Body::builder()
            .add_block(vpc_block)
            .build()
    }
}

#[test]
fn test_default_aws_vpc_generate_hcl() {
    let subnet = AwsVpc::default();
    let expected = r#"
resource "aws_vpc" "cluster_vpc" {
  cidr_block = "10.0.0.0/16"
  enable_dns_support = true
  enable_dns_hostnames = true
}
"#.trim_start();

    let hcl = hcl::to_string(&subnet.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
