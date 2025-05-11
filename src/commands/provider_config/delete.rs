use crate::database::models::ProviderConfig;
use inquire::Confirm;
use sqlx::sqlite::SqlitePool;
use tracing::{error, info};

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
    println!("\n\n=== Provider Configuration to be deleted ===");
    println!("{:<20}: {}", "ID", config_to_be_deleted.id);
    println!("{:<20}: {}", "Provider", config_to_be_deleted.provider_id);
    println!("{:<20}: {}", "Name", config_to_be_deleted.display_name);
    println!();

    // Prompt user for confirmation
    if !skip_confirmation {
        match Confirm::new("Do you want to proceed deleting this provider config?")
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

    // Delete the config
    config_to_be_deleted.delete(pool).await?;

    println!("Provider configuration and associated credentials deleted.");
    Ok(())
}
