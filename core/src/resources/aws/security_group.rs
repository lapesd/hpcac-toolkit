use hcl;
use std::fmt;

use super::base::{AwsResource, AwsResourceType};
use super::vpc::AwsVpc;


pub enum TransportProtocol {
    UDP,
    TCP,
    ANY,
}

impl fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransportProtocol::UDP => write!(f, "udp"),
            TransportProtocol::TCP => write!(f, "tcp"),
            TransportProtocol::ANY => write!(f, "-1"),
        }
    }
}

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

    pub fn generate_security_rule_block(&self) -> hcl::Block {
        hcl::Block::builder(self.rule_type.to_string())
            .add_attribute(("from_port", self.from_port))
            .add_attribute(("to_port", self.to_port))
            .add_attribute(("protocol", self.protocol.to_string()))
            .add_attribute(("cidr_blocks", hcl::Expression::from(self.cidr_blocks.to_owned())))
            .build()
    }
}

pub struct AwsSecurityGroup {
    resource_type: AwsResourceType,
    resource_name: String,
    name: String,
    description: String,
    vpc_id: hcl::RawExpression,
    ingress: SecurityRule,
    egress: SecurityRule,
}

impl AwsSecurityGroup {
    pub fn new(name: &str, description: &str, ingress: SecurityRule, egress: SecurityRule) -> Self {
        Self {
            resource_type: AwsResourceType::AwsSecurityGroup,
            resource_name: name.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            vpc_id: AwsVpc::default().get_id(),
            ingress,
            egress,
        }
    }
}

impl AwsResource for AwsSecurityGroup {
    fn get_id(&self) -> hcl::RawExpression {
        hcl::RawExpression::new(format!("{}.{}.id", self.resource_type, self.resource_name))
    }

    fn generate_hcl(&self) -> hcl::Body {
        let security_group_block = hcl::Block::builder("resource")
            .add_labels([self.resource_type.to_string(), self.resource_name.to_owned()])
            .add_attribute(("name", self.name.to_owned()))
            .add_attribute(("description", self.description.to_owned()))
            .add_attribute(("vpc_id", self.vpc_id.to_owned()))
            .add_block(self.ingress.generate_security_rule_block())
            .add_block(self.egress.generate_security_rule_block())
            .build();

        hcl::Body::builder()
            .add_block(security_group_block)
            .build()
    }
}
