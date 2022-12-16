use hcl::{Block as TerraformBlock, Body as TerraformBody, RawExpression};
use std::fs::File;
use std::io::Write;
use std::path::Path;


pub struct DeployerSshKey {
    key_name: String,
    public_key: String,
}

impl DeployerSshKey {
    pub fn new(key_name: &str, public_key: &str) -> Self {
        Self {
            key_name: key_name.to_string(),
            public_key: public_key.to_string(),
        }
    }

    pub fn generate_hcl(&self) -> TerraformBody {
        let ssh_key = TerraformBlock::builder("resource")
            .add_labels(["aws_key_pair", "deployer_key"])
            .add_attribute(("key_name", self.key_name.to_owned()))
            .add_attribute(("public_key", RawExpression::new(&format!("file(\"{}\")", self.public_key))));

        TerraformBody::builder()
            .add_block(ssh_key.build())
            .build()
    }

    pub fn save_hcl(self, terraform_dir: &str) -> std::io::Result<()> {
        let path_str = format!("{}/deployer_key.tf", terraform_dir);
        let path = Path::new(&path_str);
        let hcl_body = self.generate_hcl();
        let hcl_string = hcl::to_string(&hcl_body).unwrap();
        let mut file = File::create(path)?;
        file.write_all(hcl_string.as_bytes())?;
        Ok(())
    }
}
