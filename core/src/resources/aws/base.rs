use std::fmt;
use hcl;


pub trait AwsResource {
    fn get_id(&self) -> hcl::RawExpression;
    fn generate_hcl(&self) -> hcl::Body;
}


pub enum AwsResourceType {
    AwsVpc,
    AwsSubnet,
    AwsInternetGateway,
    AwsRouteTable,
    AwsRoute,
    AwsRouteTableAssociation,
    AwsSecurityGroup,
    AwsKeyPair,
    AwsInstance,
    AwsSpotInstanceRequest,
}

impl fmt::Display for AwsResourceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AwsResourceType::AwsVpc => write!(f, "aws_vpc"),
            AwsResourceType::AwsSubnet => write!(f, "aws_subnet"),
            AwsResourceType::AwsInternetGateway => write!(f, "aws_internet_gateway"),
            AwsResourceType::AwsRouteTable => write!(f, "aws_route_table"),
            AwsResourceType::AwsRoute => write!(f, "aws_route"),
            AwsResourceType::AwsRouteTableAssociation => write!(f, "aws_route_table_association"),
            AwsResourceType::AwsSecurityGroup => write!(f, "aws_security_group"),
            AwsResourceType::AwsKeyPair => write!(f, "aws_key_pair"),
            AwsResourceType::AwsInstance => write!(f, "aws_instance"),
            AwsResourceType::AwsSpotInstanceRequest => write!(f, "aws_spot_instance_request"),
        }
    }
}

#[test]
fn test_aws_resource_type_enum_display_trait() {
    let aws_resource_type = AwsResourceType::AwsVpc;
    assert_eq!(
        "aws_vpc".to_string(),
        format!("{}", aws_resource_type.to_string())
    )
}


pub enum AwsRegion {
    Ohio,
    NorthVirginia,
    NorthCalifornia,
    Oregon,
    CapeTown,
    HongKong,
    Jakarta,
    Mumbai,
    Osaka,
    Seoul,
    Singapore,
    Sydney,
    Tokyo,
    CentralCanada,
    Frankfurt,
    Ireland,
    London,
    Milan,
    Paris,
    Stockholm,
    Bahrain,
    UAE,
    SãoPaulo,
}

impl fmt::Display for AwsRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AwsRegion::Ohio => write!(f, "us-east-2"),
            AwsRegion::NorthVirginia => write!(f, "us-east-1"),
            AwsRegion::NorthCalifornia => write!(f, "us-west-1"),
            AwsRegion::Oregon => write!(f, "us-west-2"),
            AwsRegion::CapeTown => write!(f, "af-south-1"),
            AwsRegion::HongKong => write!(f, "ap-east-1"),
            AwsRegion::Jakarta => write!(f, "ap-southeast-3"),
            AwsRegion::Mumbai => write!(f, "ap-south-1"),
            AwsRegion::Osaka => write!(f, "ap-northeast-3"),
            AwsRegion::Seoul => write!(f, "ap-northeast-2"),
            AwsRegion::Singapore => write!(f, "ap-southeast-1"),
            AwsRegion::Sydney => write!(f, "ap-southeast-2"),
            AwsRegion::Tokyo => write!(f, "ap-northeast-1"),
            AwsRegion::CentralCanada => write!(f, "ca-central-1"),
            AwsRegion::Frankfurt => write!(f, "eu-central-1"),
            AwsRegion::Ireland => write!(f, "eu-west-1"),
            AwsRegion::London => write!(f, "eu-west-2"),
            AwsRegion::Milan => write!(f, "eu-south-1"),
            AwsRegion::Paris => write!(f, "eu-west-3"),
            AwsRegion::Stockholm => write!(f, "eu-north-1"),
            AwsRegion::Bahrain => write!(f, "me-south-1"),
            AwsRegion::UAE => write!(f, "me-central-1"),
            AwsRegion::SãoPaulo => write!(f, "sa-east-1"),
        }
    }
}

#[test]
fn test_aws_availability_zone_enum_display_trait() {
    let aws_availability_zone = AwsRegion::Frankfurt;
    assert_eq!(
        "eu-central-1".to_string(),
        format!("{}", aws_availability_zone.to_string())
    )
}
