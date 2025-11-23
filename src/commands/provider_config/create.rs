use crate::database::models::{ConfigVar, Provider, ProviderConfig};
use crate::utils;

use anyhow::{Result, bail};
use inquire::{Select, Text};
use sqlx::sqlite::SqlitePool;
use tracing::error;

pub async fn create(pool: &SqlitePool, skip_confirmation: bool) -> Result<()> {
    let providers = Provider::fetch_all(pool).await?;
    if providers.is_empty() {
        bail!("Providers table is empty, please check SQLite seed data");
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
            error!("{}", e.to_string());
            bail!("Failed processing user selection")
        }
    };

    let display_name: String = Text::new("Enter a name for your provider config:").prompt()?;
    if !display_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        bail!(
            "Invalid display_name `{}` contains invalid characters.",
            display_name
        )
    };

    let mut config_vars: Vec<ConfigVar> = vec![];
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

    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed creating this provider configuration?",
    )?) {
        return Ok(());
    }

    ProviderConfig::insert(pool, display_name, provider.id.clone(), config_vars).await?;

    println!("New provider configuration created successfully!");
    Ok(())
}
