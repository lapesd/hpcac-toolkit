use crate::database::models::{ConfigVar, Provider, ProviderConfig};
use inquire::{Confirm, Password, Select, Text};
use sqlx::sqlite::SqlitePool;
use tracing::{error, info};

pub async fn create(pool: &SqlitePool, skip_confirmation: bool) -> anyhow::Result<()> {
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
            error!("{}", e.to_string());
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
    let required_keys = provider.get_required_config_vars();
    for key in required_keys {
        let value = Password::new(&format!("Enter value for {}:", key))
            .without_confirmation()
            .with_display_toggle_enabled()
            .prompt()?;
        config_vars.push(ConfigVar {
            id: 0,
            provider_config_id: 0,
            key,
            value,
        });
    }

    if !skip_confirmation {
        match Confirm::new("Do you want to proceed with adding this provider config?")
            .with_default(true)
            .prompt()
        {
            Ok(true) => {}
            Ok(false) => {
                println!("Operation cancelled by user");
                return Ok(());
            }
            Err(e) => {
                error!("{}", e.to_string());
                anyhow::bail!("Error processing user response")
            }
        }
    } else {
        info!("Automatic confirmation with -y flag. Proceeding...");
    }

    ProviderConfig::insert(pool, display_name, provider.id.clone(), config_vars).await?;

    println!("New provider configuration created successfully!");
    Ok(())
}
