use hcl;

use super::base::{AwsRegion, AwsResource, AwsResourceType};
use super::vpc::AwsVpc;


pub struct AwsSubnet {
    resource_type: AwsResourceType,
    resource_name: String,
    vpc_id: hcl::RawExpression,
    cidr_block: String,
    availability_zone: String,
    map_public_ip_on_launch: bool,
}

impl Default for AwsSubnet {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsSubnet,
            resource_name: "cluster_subnet".to_string(),
            vpc_id: AwsVpc::default().get_id(),
            cidr_block: "10.0.0.0/20".to_string(),
            availability_zone: format!("{}a", AwsRegion::NorthVirginia),
            map_public_ip_on_launch: true,
        }
    }
}

impl AwsResource for AwsSubnet {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))    
    }

    fn generate_hcl(&self) -> hcl::Body {
        let subnet_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("vpc_id", self.vpc_id.to_owned()))
            .add_attribute(("cidr_block", self.cidr_block.to_owned()))
            .add_attribute(("availability_zone", self.availability_zone.to_owned()))
            .add_attribute(("map_public_ip_on_launch", self.map_public_ip_on_launch))
            .build();

        hcl::Body::builder()
            .add_block(subnet_block)
            .build()
    }
}

#[test]
fn test_default_aws_subnet_generate_hcl() {
    let subnet = AwsSubnet::default();
    let expected = r#"
resource "aws_subnet" "cluster_subnet" {
  vpc_id = aws_vpc.cluster_vpc.id
  cidr_block = "10.0.0.0/20"
  availability_zone = "us-east-1a"
  map_public_ip_on_launch = true
}
"#.trim_start();

    let hcl = hcl::to_string(&subnet.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
