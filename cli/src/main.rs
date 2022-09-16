use hpcc_core::providers::aws::plugin::AwsPluginOptions;
use hpcc_core::providers::aws::vpc::VPC;
use hpcc_core::terraform::TerraformProvidersOptions;

fn main() {
    dotenv::dotenv().ok();

    let terraform_dir = std::env::var("TERRAFORM_DIR").unwrap();

    // Example of basic Terraform setup
    let terraform_providers_config = TerraformProvidersOptions::default();
    terraform_providers_config.save_hcl(&terraform_dir).unwrap();

    // Example of an AWS provider configuration
    let access_key = std::env::var("ACCESS_KEY").unwrap();
    let secret_key = std::env::var("SECRET_KEY").unwrap();
    let aws_provider_config = AwsPluginOptions::default(&access_key, &secret_key);
    aws_provider_config.save_hcl(&terraform_dir).unwrap();

    // Example of base VPC configuration
    let vpc = VPC::default(&aws_provider_config);
    vpc.save_hcl(&terraform_dir).unwrap();
}
