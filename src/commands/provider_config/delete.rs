use crate::database::models::ProviderConfig;
use crate::utils;
use sqlx::sqlite::SqlitePool;

pub async fn delete(
    pool: &SqlitePool,
    provider_config_id: &str,
    skip_confirmation: bool,
) -> anyhow::Result<()> {
    // Parse the string into an i64
    let id = provider_config_id
        .parse::<i64>()
        .map_err(|_| anyhow::anyhow!("Invalid provider config ID, must be a valid integer"))?;

    // Fetch the config to be deleted
    let config_to_be_deleted = ProviderConfig::fetch_by_id(pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Provider config with ID {} not found", id))?;

    // Print ProviderConfig details
    println!("\n=== Provider Configuration to be deleted ===");
    println!("{:<20}: {}", "ID", config_to_be_deleted.id);
    println!("{:<20}: {}", "Provider", config_to_be_deleted.provider_id);
    println!("{:<20}: {}", "Name", config_to_be_deleted.display_name);
    println!();

    // Prompt user for confirmation
    if !(utils::user_confirmation(
        skip_confirmation,
        "Do you want to proceed creating this provider configuration?",
    )?) {
        return Ok(());
    }

    // Delete the config
    config_to_be_deleted.delete(pool).await?;

    println!("Provider configuration and associated credentials deleted.");
    Ok(())
}
