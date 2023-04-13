use std::env;
use std::path::Path;
use std::process::exit;

use colored::Colorize;
use inquire::Select;
use sqlx::PgPool;

use hpcac_core::models::provider::Provider;

#[derive(Debug)]
pub struct CliConfig {
    pub terraform_dir: String,
    pub ssh_key_name: String,
    pub ssh_public_key_path: String,
    pub pg_uri: String,
}

pub fn _read_optional_var(var_name: &str) -> Option<String> {
    match env::var(var_name) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

pub fn read_required_var(var_name: &str) -> String {
    match env::var(var_name) {
        Ok(value) => value,
        Err(_) => {
            println!(
                "{}: required {} environment variable not set.\n",
                "ERROR".red().bold(),
                var_name.bold()
            );
            exit(0);
        }
    }
}

pub fn load_config_from_environment_variables() -> CliConfig {
    // Read Environment variables from .env file
    dotenv::dotenv().ok();

    // Check if provided TERRAFORM_DIR exists in the system
    let terraform_dir = read_required_var("TERRAFORM_DIR");
    match Path::new(&terraform_dir).is_dir() {
        true => {}
        false => {
            println!(
                "{}: provided TERRAFORM_DIR `{}` doesn't exist.\n",
                "ERROR".red().bold(),
                &terraform_dir.bold()
            );
            exit(0);
        }
    }

    // Build CliConfig
    CliConfig {
        terraform_dir,
        ssh_key_name: read_required_var("SSH_KEY_NAME"),
        ssh_public_key_path: read_required_var("SSH_PUBLIC_KEY_PATH"),
        pg_uri: read_required_var("PG_URI"),
    }
}

pub async fn provider_selector(pool: &PgPool) -> String {
    // PROMPT user for desired Cloud Provider
    let provider_options: Vec<String> = match Provider::fetch_all(pool).await {
        Ok(providers) => providers
            .iter()
            .map(|provider| provider.alias.to_owned())
            .collect(),
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };

    let selected_provider = match Select::new("Select cloud provider:", provider_options).prompt() {
        Ok(provider) => match provider.as_str() {
            "aws" => provider,
            _ => {
                println!(
                    "{}: this provider is currently not fully supported.\n",
                    "ERROR".red().bold()
                );
                exit(0);
            }
        },
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };

    return selected_provider;
}
