use hcl;

use super::base::{AwsResource, AwsResourceType};
use super::vpc::AwsVpc;


pub struct AwsInternetGateway {
    resource_type: AwsResourceType,
    resource_name: String,
    vpc_id: hcl::RawExpression,
}

impl Default for AwsInternetGateway {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsInternetGateway,
            resource_name: "cluster_ig".to_string(),
            vpc_id: AwsVpc::default().get_id(),
        }
    }
}

impl AwsResource for AwsInternetGateway {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))    
    }

    fn generate_hcl(&self) -> hcl::Body {
        let subnet_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("vpc_id", self.vpc_id.to_owned()))
            .build();

        hcl::Body::builder()
            .add_block(subnet_block)
            .build()
    }
}

#[test]
fn test_default_aws_internet_gateway_generate_hcl() {
    let internet_gateway = AwsInternetGateway::default();
    let expected = r#"
resource "aws_internet_gateway" "cluster_ig" {
  vpc_id = aws_vpc.cluster_vpc.id
}
"#.trim_start();

    let hcl = hcl::to_string(&internet_gateway.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
