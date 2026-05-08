use crate::database::models::{ConfigVar, Provider, ProviderConfig};
use crate::utils;

use anyhow::Result;
use inquire::{Select, Text};
use sqlx::sqlite::SqlitePool;

const AUTH_MODE_MANUAL: &str = "Manual credentials (Access Key ID + Secret Access Key)";
const AUTH_MODE_CHAIN: &str = "AWS credential chain (uses ~/.aws/credentials, SSO, env vars, etc.)";

pub async fn create(pool: &SqlitePool, skip_confirmation: bool) -> Result<()> {
    let providers = Provider::fetch_all(pool).await?;
    if providers.is_empty() {
        anyhow::bail!("Providers table is empty, please check SQLite seed data");
    }

    let provider_options: Vec<&str> = providers.iter().map(|p| p.display_name.as_str()).collect();
    let provider = match Select::new(
        "Select a cloud provider to configure credentials:\n",
        provider_options,
    )
    .without_filtering()
    .prompt()
    {
        Ok(selection) => providers
            .iter()
            .find(|p| p.display_name == selection)
            .expect("Selected provider not found"),
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!("Failed processing user selection")
        }
    };

    let display_name: String = Text::new("Enter a name for your provider config:").prompt()?;
    if !display_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        anyhow::bail!(
            "Invalid display_name `{}` contains invalid characters.",
            display_name
        )
    };

    let mut config_vars: Vec<ConfigVar> = vec![];

    // For AWS, offer a choice between manual credentials and the AWS credential chain
    let use_chain = if provider.id == "aws" {
        let auth_mode = match Select::new(
            "Select authentication mode:\n",
            vec![AUTH_MODE_MANUAL, AUTH_MODE_CHAIN],
        )
        .without_filtering()
        .prompt()
        {
            Ok(selection) => selection,
            Err(e) => {
                tracing::error!("{}", e.to_string());
                anyhow::bail!("Failed processing user selection")
            }
        };
        auth_mode == AUTH_MODE_CHAIN
    } else {
        false
    };

    if use_chain {
        config_vars.push(ConfigVar {
            id: 0,
            provider_config_id: 0,
            key: "AUTH_MODE".to_string(),
            value: "chain".to_string(),
        });
    } else {
        let required_keys = provider.get_required_config_vars();
        for key in required_keys {
            let value = Text::new(&format!("Enter value for {}:", key)).prompt()?;
            config_vars.push(ConfigVar {
                id: 0,
                provider_config_id: 0,
                key,
                value,
            });
        }

        let optional_keys = provider.get_optional_config_vars();
        for key in optional_keys {
            let value = Text::new(&format!(
                "Enter value for {} (optional, press Enter to skip):",
                key
            ))
            .prompt()?;
            if !value.trim().is_empty() {
                config_vars.push(ConfigVar {
                    id: 0,
                    provider_config_id: 0,
                    key,
                    value,
                });
            }
        }
    }

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed creating this provider configuration?",
    )?) {
        return Ok(());
    }

    ProviderConfig::insert(pool, display_name, provider.id.clone(), config_vars).await?;

    tracing::info!("New provider configuration created successfully!");
    Ok(())
}
