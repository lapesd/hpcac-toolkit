use hcl;

use super::base::{AwsResource, AwsResourceType};
use super::vpc::AwsVpc;


pub struct AwsRouteTable {
    resource_type: AwsResourceType,
    resource_name: String,
    vpc_id: hcl::RawExpression,
}

impl Default for AwsRouteTable {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsRouteTable,
            resource_name: "cluster_rt".to_string(),
            vpc_id: AwsVpc::default().get_id(),
        }
    }
}

impl AwsResource for AwsRouteTable {
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
fn test_default_aws_route_table_generate_hcl() {
    let route_table = AwsRouteTable::default();
    let expected = r#"
resource "aws_route_table" "cluster_rt" {
  vpc_id = aws_vpc.cluster_vpc.id
}
"#.trim_start();

    let hcl = hcl::to_string(&route_table.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
