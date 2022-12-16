use hcl;

use super::base::{AwsResource, AwsResourceType};
use super::route_table::AwsRouteTable;
use super::subnet::AwsSubnet;


pub struct AwsRouteTableAssociation {
    resource_type: AwsResourceType,
    resource_name: String,
    route_table_id: hcl::RawExpression,
    subnet_id: hcl::RawExpression,
}

impl Default for AwsRouteTableAssociation {
    fn default() -> Self {
        Self {
            resource_type: AwsResourceType::AwsRouteTableAssociation,
            resource_name: "cluster_rt_association".to_string(),
            route_table_id: AwsRouteTable::default().get_id(),
            subnet_id: AwsSubnet::default().get_id(),
        }
    }
}

impl AwsResource for AwsRouteTableAssociation {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))    
    }

    fn generate_hcl(&self) -> hcl::Body {
        let subnet_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("route_table_id", self.route_table_id.to_owned()))
            .add_attribute(("subnet_id", self.subnet_id.to_owned()))
            .build();

        hcl::Body::builder()
            .add_block(subnet_block)
            .build()
    }
}

#[test]
fn test_default_aws_route_generate_hcl() {
    let route = AwsRouteTableAssociation::default();
    let expected = r#"
resource "aws_route_table_association" "cluster_rt_association" {
  route_table_id = aws_route_table.cluster_rt.id
  subnet_id = aws_subnet.cluster_subnet.id
}
"#.trim_start();

    let hcl = hcl::to_string(&route.generate_hcl()).unwrap();
    assert_eq!(expected, hcl);
}
