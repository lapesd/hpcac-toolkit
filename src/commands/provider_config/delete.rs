use crate::database::models::ProviderConfig;
use crate::utils;

use anyhow::Result;
use sqlx::sqlite::SqlitePool;

pub async fn delete(
    pool: &SqlitePool,
    provider_config_id: &str,
    skip_confirmation: bool,
) -> Result<()> {
    let id = match provider_config_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => {
            anyhow::bail!("Invalid Provider configuration ID, must be a valid integer");
        }
    };

    let config = match ProviderConfig::fetch_by_id(pool, id).await {
        Ok(Some(config)) => config,
        Ok(None) => {
            anyhow::bail!("Provider configuration (id='{}') not found", id);
        }
        Err(e) => {
            tracing::error!("{}", e.to_string());
            anyhow::bail!("DB Operation Failure: {}", e)
        }
    };

    tracing::info!(
        "\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}",
        "Provider Configuration Name",
        config.display_name,
        "ID",
        config.id,
        "Provider",
        config.provider_id
    );

    if !(utils::user_confirmation(
        skip_confirmation,
        "Confirm deleting this Provider configuration?",
    )?) {
        return Ok(());
    }

    config.delete(pool).await?;

    tracing::info!("Provider configuration and associated credentials deleted.");
    Ok(())
}
