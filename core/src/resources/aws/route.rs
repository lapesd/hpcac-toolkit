use hcl;

use super::base::{AwsResource, AwsResourceType};
use super::internet_gateway::AwsInternetGateway;
use super::route_table::AwsRouteTable;


pub struct AwsRoute {
    resource_type: AwsResourceType,
    resource_name: String,
    route_table_id: hcl::RawExpression,
    gateway_id: hcl::RawExpression,
    destination_cidr_block: String,
}

impl Default for AwsRoute {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsRoute,
            resource_name: "cluster_r".to_string(),
            route_table_id: AwsRouteTable::default().get_id(),
            gateway_id: AwsInternetGateway::default().get_id(),
            destination_cidr_block: "0.0.0.0/0".to_string(),
        }
    }
}

impl AwsResource for AwsRoute {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))    
    }

    fn generate_hcl(&self) -> hcl::Body {
        let subnet_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("route_table_id", self.route_table_id.to_owned()))
            .add_attribute(("gateway_id", self.gateway_id.to_owned()))
            .add_attribute(("destination_cidr_block", self.destination_cidr_block.to_owned()))
            .build();

        hcl::Body::builder()
            .add_block(subnet_block)
            .build()
    }
}

#[test]
fn test_default_aws_route_generate_hcl() {
    let route = AwsRoute::default();
    let expected = r#"
resource "aws_route" "cluster_r" {
  route_table_id = aws_route_table.cluster_rt.id
  gateway_id = aws_internet_gateway.cluster_ig.id
  destination_cidr_block = "0.0.0.0/0"
}
"#.trim_start();

    let hcl = hcl::to_string(&route.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
