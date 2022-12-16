use hcl::Block as TerraformBlock;


pub struct RootBlockStorageDevice {
    delete_on_termination: bool,
    volume_size: i32
}

impl RootBlockStorageDevice {
    pub fn default() -> Self {
        Self {
            delete_on_termination: true,
            volume_size: 10,
        }
    }

    pub fn generate_hcl_block(&self) -> TerraformBlock {
        TerraformBlock::builder("root_block_device")
            .add_attribute(("delete_on_termination", self.delete_on_termination))
            .add_attribute(("volume_size", self.volume_size))
            .build()
    }
}

pub struct ElasticBlockStorageDevice {
    delete_on_termination: bool,
    device_name: String,
    volume_size: i32
}

impl ElasticBlockStorageDevice {
    pub fn default(device_name: &str) -> Self {
        Self {
            delete_on_termination: true,
            device_name: device_name.to_string(),
            volume_size: 10,
        }
    }

    pub fn generate_hcl_block(&self) -> TerraformBlock {
        TerraformBlock::builder("ebs_block_device")
            .add_attribute(("delete_on_termination", self.delete_on_termination))
            .add_attribute(("device_name", self.device_name.to_owned()))
            .add_attribute(("volume_size", self.volume_size))
            .build()
    }
}
