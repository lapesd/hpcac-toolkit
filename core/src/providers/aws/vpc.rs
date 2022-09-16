use hcl::{Block as TerraformBlock, Body as TerraformBody, Expression, RawExpression};
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::providers::common::TransportProtocol;
use super::plugin::AwsPluginOptions;

pub enum SecurityRuleType {
    Ingress,
    Egress,
}

impl fmt::Display for SecurityRuleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SecurityRuleType::Ingress => write!(f, "ingress"),
            SecurityRuleType::Egress => write!(f, "egress"),
        }
    }
}

pub struct SecurityRule {
    rule_type: SecurityRuleType,
    from_port: u64,
    to_port: u64,
    protocol: TransportProtocol,
    cidr_blocks: Vec<String>,
}

impl SecurityRule {
    pub fn new(
        rule_type: SecurityRuleType,
        from_port: u64,
        to_port: u64,
        protocol: TransportProtocol,
    ) -> Self {
        Self {
            rule_type,
            from_port,
            to_port,
            protocol,
            cidr_blocks: vec!["0.0.0.0/0".to_string()],
        }
    }

    pub fn generate_hcl_block(&self) -> TerraformBlock {
        TerraformBlock::builder(self.rule_type.to_string())
            .add_attribute(("from_port", self.from_port))
            .add_attribute(("to_port", self.to_port))
            .add_attribute(("protocol", self.protocol.to_string()))
            .add_attribute(("cidr_blocks", Expression::from(self.cidr_blocks.to_owned())))
            .build()
    }
}

pub struct SecurityGroup {
    name: String,
    description: String,
    vpc_id: String,
    ingress: SecurityRule,
    egress: SecurityRule,
}

impl SecurityGroup {
    pub fn new(name: &str, description: &str, ingress: SecurityRule, egress: SecurityRule) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            vpc_id: "aws_vpc.cluster_vpc.id".to_string(),
            ingress,
            egress,
        }
    }

    pub fn generate_hcl_block(&self) -> TerraformBlock {
        TerraformBlock::builder("resource")
            .add_labels(["aws_security_group", &self.name])
            .add_attribute(("name", self.name.to_owned()))
            .add_attribute(("description", self.description.to_owned()))
            .add_attribute(("vpc_id", RawExpression::new(self.vpc_id.to_owned())))
            .add_block(self.ingress.generate_hcl_block())
            .add_block(self.egress.generate_hcl_block())
            .build()
    }
}

#[derive(Debug)]
pub struct Subnet {
    vpc_id: String,
    cidr_block: String,
    availability_zone: String,
    map_public_ip_on_launch: bool,
}

impl Subnet {
    pub fn default(aws_options: AwsPluginOptions) -> Self {
        Self {
            vpc_id: "aws_vpc.cluster_vpc.id".to_string(),
            cidr_block: "10.0.0.0/20".to_string(),
            availability_zone: format!("{}a", aws_options.region),
            map_public_ip_on_launch: true,
        }
    }

    pub fn generate_hcl_block(&self) -> TerraformBlock {
        TerraformBlock::builder("resource")
            .add_labels(["aws_subnet", "cluster_subnet"])
            .add_attribute(("vpc_id", RawExpression::new(&self.vpc_id)))
            .add_attribute(("cidr_block", self.cidr_block.to_owned()))
            .add_attribute(("availability_zone", self.availability_zone.to_owned()))
            .add_attribute(("map_public_ip_on_launch", self.map_public_ip_on_launch))
            .build()
    }
}

#[derive(Debug)]
pub enum InstanceTenancy {
    Default,
    Dedicated,
}

impl fmt::Display for InstanceTenancy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InstanceTenancy::Default => write!(f, "default"),
            InstanceTenancy::Dedicated => write!(f, "dedicated"),
        }
    }
}

pub struct VPC {
    aws_options: AwsPluginOptions,
    cidr_block: String,
    instance_tenancy: InstanceTenancy,
    enable_dns_support: bool,
    enable_dns_hostnames: bool,
}

impl VPC {
    pub fn default(aws_options: &AwsPluginOptions) -> Self {
        Self {
            aws_options: aws_options.clone(),
            cidr_block: "10.0.0.0/16".to_string(),
            instance_tenancy: InstanceTenancy::Default,
            enable_dns_support: true,
            enable_dns_hostnames: true,
        }
    }

    fn generate_hcl(&self) -> TerraformBody {
        let vpc = TerraformBlock::builder("resource")
            .add_labels(["aws_vpc", "cluster_vpc"])
            .add_attribute(("cidr_block", self.cidr_block.to_owned()))
            .add_attribute(("instance_tenancy", self.instance_tenancy.to_string()))
            .add_attribute(("enable_dns_support", self.enable_dns_support))
            .add_attribute(("enable_dns_hostnames", self.enable_dns_hostnames));

        let subnet = Subnet::default(self.aws_options.clone());

        let internet_gateway = TerraformBlock::builder("resource")
            .add_labels(["aws_internet_gateway", "cluster_ig"])
            .add_attribute(("vpc_id", RawExpression::new(subnet.vpc_id.to_owned())));

        let route_table = TerraformBlock::builder("resource")
            .add_labels(["aws_route_table", "cluster_rt"])
            .add_attribute(("vpc_id", RawExpression::new(subnet.vpc_id.to_owned())));

        let route = TerraformBlock::builder("resource")
            .add_labels(["aws_route", "cluster_r"])
            .add_attribute(("route_table_id", RawExpression::new("aws_route_table.cluster_rt.id")))
            .add_attribute(("destination_cidr_block", "0.0.0.0/0"))
            .add_attribute(("gateway_id", RawExpression::new("aws_internet_gateway.cluster_ig.id")));

        let route_table_association = TerraformBlock::builder("resource")
            .add_labels(["aws_route_table_association", "cluster_rta"])
            .add_attribute(("subnet_id", RawExpression::new("aws_subnet.cluster_subnet.id")))
            .add_attribute(("route_table_id", RawExpression::new("aws_route_table.cluster_rt.id")));

        let ssh_sg = SecurityGroup::new(
            "allow_ssh",
            "Allow SSH traffic",
            SecurityRule::new(
                SecurityRuleType::Ingress, 
                22, 
                22, 
                TransportProtocol::TCP
            ),
            SecurityRule::new(
                SecurityRuleType::Egress, 
                0, 
                0, 
                TransportProtocol::ANY
            ),
        );

        let nfs_sg = SecurityGroup::new(
            "allow_nfs",
            "Allow NFS traffic",
            SecurityRule::new(
                SecurityRuleType::Ingress,
                2049,
                2049,
                TransportProtocol::TCP,
            ),
            SecurityRule::new(
                SecurityRuleType::Egress, 
                0, 
                0, 
                TransportProtocol::ANY
            ),
        );

        let mpi_sg = SecurityGroup::new(
            "allow_mpi",
            "Allow MPI traffic",
            SecurityRule::new(
                SecurityRuleType::Ingress, 
                0, 
                65535, 
                TransportProtocol::TCP
            ),
            SecurityRule::new(
                SecurityRuleType::Egress, 
                0, 
                65535, 
                TransportProtocol::TCP
            ),
        );

        TerraformBody::builder()
            .add_block(vpc.build())
            .add_block(subnet.generate_hcl_block())
            .add_block(internet_gateway.build())
            .add_block(route_table.build())
            .add_block(route.build())
            .add_block(route_table_association.build())
            .add_block(ssh_sg.generate_hcl_block())
            .add_block(nfs_sg.generate_hcl_block())
            .add_block(mpi_sg.generate_hcl_block())
            .build()
    }

    pub fn save_hcl(self, terraform_dir: &str) -> std::io::Result<()> {
        let path_str = format!("{}/vpc.tf", terraform_dir);
        let path = Path::new(&path_str);
        let hcl_body = self.generate_hcl();
        let hcl_string = hcl::to_string(&hcl_body).unwrap();
        let mut file = File::create(path)?;
        file.write_all(hcl_string.as_bytes())?;
        Ok(())
    }
}
