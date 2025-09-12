use crate::database::models::ProviderConfig;
use crate::utils;

use anyhow::{Result, bail};
use sqlx::sqlite::SqlitePool;
use tracing::error;

pub async fn delete(
    pool: &SqlitePool,
    provider_config_id: &str,
    skip_confirmation: bool,
) -> Result<()> {
    let id = match provider_config_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            bail!("Invalid Provider configuration ID, must be a valid integer");
        }
    };

    let config = match ProviderConfig::fetch_by_id(pool, id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            bail!("Provider configuration (id='{}') not found", id);
        }
        Err(e) => {
            error!("{}", e.to_string());
            bail!("DB Operation Failure")
        }
    };

    println!(
        "\n{:<35}: {}",
        "Provider Configuration Name", config.display_name
    );
    println!("{:<35}: {}", "ID", config.id);
    println!("{:<35}: {}", "Provider\n", config.provider_id);

    if !(utils::user_confirmation(
        skip_confirmation,
        "Confirm deleting this Provider configuration?",
    )?) {
        return Ok(());
    }

    config.delete(pool).await?;

    println!("Provider configuration and associated credentials deleted.");
    Ok(())
}
