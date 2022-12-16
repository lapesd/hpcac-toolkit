use hpcac_core::providers::aws::plugin::AwsPluginOptions;
use hpcac_core::providers::aws::ssh_key::DeployerSshKey;
use hpcac_core::providers::aws::vpc::VPC;
use hpcac_core::providers::aws::instance::{AwsInstance, InstanceType, NodeSize};
use hpcac_core::terraform::TerraformProvidersOptions;


pub async fn create(provider: &str, flavor: &str, size: u8) {
    println!("{} {} {}", provider, size, flavor);
    
    dotenv::dotenv().ok();

    let terraform_dir = std::env::var("TERRAFORM_DIR").unwrap();
    
    // Terraform setup
    let terraform_providers_config = TerraformProvidersOptions::default();
    terraform_providers_config.save_hcl(&terraform_dir).unwrap();
    
    // AWS provider configuration
    let access_key = std::env::var("ACCESS_KEY").unwrap();
    let secret_key = std::env::var("SECRET_KEY").unwrap();
    let aws_provider_config = AwsPluginOptions::default(&access_key, &secret_key);
    aws_provider_config.save_hcl(&terraform_dir).unwrap();
    
    // VPC configuration
    let vpc = VPC::default(&aws_provider_config);
    vpc.save_hcl(&terraform_dir).unwrap();

    // SSH Deployer key configuration
    let ssh_public_key_path = "~/.ssh/id_rsa.pub";
    let ssh_key_name = "vnderlev@DESKTOP-FIQR0CK";
    let key = DeployerSshKey::new(ssh_key_name, ssh_public_key_path);
    key.save_hcl(&terraform_dir).unwrap();

    // Cluster terraform generation
    let i1 = AwsInstance::default(
        "ami-08e4e35cccc6189f4", 
        "head", InstanceType::T2(NodeSize::Nano), 
        "10.0.0.10");
    i1.save_hcl(&terraform_dir).unwrap();

    let i2 = AwsInstance::default(
        "ami-08e4e35cccc6189f4", 
        "worker", InstanceType::T2(NodeSize::Nano), 
        "10.0.0.11");
    i2.save_hcl(&terraform_dir).unwrap();

}
