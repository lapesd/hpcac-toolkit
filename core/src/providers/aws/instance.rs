use hcl::{Block as TerraformBlock, Expression, RawExpression};
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use super::storage::{RootBlockStorageDevice, ElasticBlockStorageDevice};


pub enum InstanceType {
    M6in(NodeSize),
    T4g(NodeSize),
    T3(NodeSize),
    T3a(NodeSize),
    T2(NodeSize),
    M6g(NodeSize),
    M6i(NodeSize),
    M6a(NodeSize),
    M5(NodeSize),
    M5a(NodeSize),
    M5n(NodeSize),
    M5zn(NodeSize),
    M4(NodeSize),
}

impl fmt::Display for InstanceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InstanceType::M6in(size) => write!(f, "m6in.{}", size),
            InstanceType::T4g(size) => write!(f, "t4g.{}", size),
            InstanceType::T3(size) => write!(f, "t3.{}", size),
            InstanceType::T2(size) => write!(f, "t2.{}", size),
            InstanceType::T3a(size) => write!(f, "t3a.{}", size),
            InstanceType::M6g(size) => write!(f, "m6g.{}", size),
            InstanceType::M6i(size) => write!(f, "m6i.{}", size),
            InstanceType::M6a(size) => write!(f, "m6a.{}", size),
            InstanceType::M5(size) => write!(f, "m5.{}", size),
            InstanceType::M5a(size) => write!(f, "m5a.{}", size),
            InstanceType::M5n(size) => write!(f, "m5n.{}", size),
            InstanceType::M5zn(size) => write!(f, "m5zn.{}", size),
            InstanceType::M4(size) => write!(f, "m4.{}", size),
        }
    }
}

pub enum NodeSize {
    Nano,
    Micro,
    Small,
    Medium,
    Large,
    Xlarge,
    XXLarge,
}

impl fmt::Display for NodeSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NodeSize::Nano => write!(f, "nano"),
            NodeSize::Micro => write!(f, "micro"),
            NodeSize::Small => write!(f, "small"),
            NodeSize::Medium => write!(f, "medium"),
            NodeSize::Large => write!(f, "large"),
            NodeSize::Xlarge => write!(f, "xlarge"),
            NodeSize::XXLarge => write!(f, "2xlarge"),
        }
    }
}

pub struct AwsInstance {
    ami: String,
    instance_name: String,
    instance_type: InstanceType,
    subnet_id: String,
    key_name: String,
    root_block_device: RootBlockStorageDevice,
    ebs_block_device: ElasticBlockStorageDevice,
    private_ip: String,
}

impl AwsInstance {
    pub fn default(ami: &str, instance_name: &str, instance_type: InstanceType, private_ip: &str) -> Self {
        Self {
            ami: ami.to_string(),
            instance_name: instance_name.to_string(),
            instance_type,
            subnet_id: "aws_subnet.cluster_subnet.id".to_string(),
            key_name: "aws_key_pair.deployer_key.key_name".to_string(),
            root_block_device: RootBlockStorageDevice::default(),
            ebs_block_device: ElasticBlockStorageDevice::default("/dev/sdh"),
            private_ip: private_ip.to_string(),
        }
    }

    fn generate_hcl(&self) -> TerraformBlock {
        return TerraformBlock::builder("resource")
            .add_labels(["aws_instance", &self.instance_name])
            .add_attribute(("ami", self.ami.to_owned()))
            .add_attribute(("instance_type", self.instance_type.to_string()))
            .add_attribute(("security_groups", Expression::from(
                vec![
                    RawExpression::new("aws_security_group.allow_ssh.id".to_string()), 
                    RawExpression::new("aws_security_group.allow_nfs.id".to_string()), 
                    RawExpression::new("aws_security_group.allow_mpi.id".to_string())
                ].to_owned()
            )))
            .add_attribute(("subnet_id", RawExpression::new(self.subnet_id.to_owned())))
            .add_attribute(("key_name", RawExpression::new(self.key_name.to_owned())))
            .add_block(self.root_block_device.generate_hcl_block())
            .add_block(self.ebs_block_device.generate_hcl_block())
            .add_attribute(("private_ip", self.private_ip.to_owned()))
            .add_attribute(("depends_on", Expression::from(
                vec![
                    RawExpression::new("aws_internet_gateway.cluster_ig")
                ]
            )))
            .build()
    }

    pub fn save_hcl(self, terraform_dir: &str) -> std::io::Result<()> {
        let path_str = format!("{}/instance_{}.tf", terraform_dir, self.instance_name);
        let path = Path::new(&path_str);
        let hcl_body = self.generate_hcl();
        let hcl_string = hcl::to_string(&hcl_body).unwrap();
        let mut file = File::create(path)?;
        file.write_all(hcl_string.as_bytes())?;
        Ok(())
    }

}
