use crate::database::models::{InstanceType, Provider, ProviderConfig};
use crate::integrations::{
    CloudInfoProvider, CloudProvider,
    providers::{aws::AwsInterface, vultr::VultrInterface},
};
use crate::utils;

use inquire::Select;
use sqlx::sqlite::SqlitePool;
use tracing::error;
use std::sync::Arc;

pub async fn load(
    pool: &SqlitePool,
    provider_id: Option<String>,
    provider_config_id: Option<String>,
    region: Option<String>,
) -> anyhow::Result<()> {
    let provider_config = match provider_config_id {
        Some(config_id) => {
            let config_id_parsed = match config_id.parse::<i64>() {
                Ok(id) => id,
                Err(e) => {
                    error!("{}", e.to_string());
                    anyhow::bail!(
                        "Invalid Provider Configuration ID: '{}' is not a valid number",
                        config_id
                    )
                }
            };
            let config_query = ProviderConfig::fetch_by_id(pool, config_id_parsed).await?;
            match config_query {
                Some(result) => result,
                None => {
                    anyhow::bail!("Provider Configuration '{}' not found", config_id_parsed)
                }
            }
        }
        None => {
            let provider = match provider_id {
                Some(provider_id) => {
                    let provider_query = Provider::fetch_by_id(pool, provider_id.clone()).await?;
                    match provider_query {
                        Some(result) => result,
                        None => {
                            anyhow::bail!("Provider '{}' not found", provider_id)
                        }
                    }
                }
                None => {
                    let mut providers = Provider::fetch_all(pool).await?;
                    if providers.is_empty() {
                        anyhow::bail!("Providers table is empty")
                    } else if providers.len() == 1 {
                        // Use the only option available
                        providers.swap_remove(0)
                    } else {
                        let provider_options: Vec<&str> =
                            providers.iter().map(|p| p.display_name.as_str()).collect();
                        let selected_provider =
                            match Select::new("Select a provider:\n", provider_options)
                                .without_filtering()
                                .prompt()
                            {
                                Ok(selection) => selection,
                                Err(e) => {
                                    error!("{}", e.to_string());
                                    anyhow::bail!("Failed processing user selection")
                                }
                            };

                        let selected_index = providers
                            .iter()
                            .position(|p| p.display_name == selected_provider)
                            .unwrap();

                        providers.swap_remove(selected_index)
                    }
                }
            };

            let mut configs = ProviderConfig::fetch_all_by_provider(pool, &provider.id).await?;
            if configs.is_empty() {
                anyhow::bail!("No provider configuration found for {}", &provider.id)
            } else if configs.len() == 1 {
                // Use the only config available
                configs.swap_remove(0)
            } else {
                let config_options: Vec<&str> =
                    configs.iter().map(|p| p.display_name.as_str()).collect();
                let selected_config =
                    match Select::new("Select a provider configuration:\n", config_options)
                        .without_filtering()
                        .prompt()
                    {
                        Ok(selection) => selection,
                        Err(e) => {
                            error!("{}", e.to_string());
                            anyhow::bail!("Failed processing user selection")
                        }
                    };

                let selected_index = configs
                    .iter()
                    .position(|p| p.display_name == selected_config)
                    .unwrap();

                configs.swap_remove(selected_index)
            }
        }
    };

    let config_vars = provider_config.get_config_vars(pool).await?;
    let provider_id = provider_config.provider_id.clone();
    let cloud_interface = match provider_id.as_str() {
        "aws" => CloudProvider::Aws(AwsInterface { config_vars, db_pool: Arc::new(pool.clone()) }),
        "vultr" => CloudProvider::Vultr(VultrInterface { config_vars }),
        _ => {
            anyhow::bail!("Provider '{}' is currently not supported.", &provider_id)
        }
    };

    println!(
        "Loading instance_types from provider '{}' using configuration '{}'...",
        provider_id, provider_config.display_name,
    );

    let regions: Vec<String> = match region {
        Some(region) => {
            println!("Selected region: '{}'", region);
            vec![region]
        }
        None => {
            let regions_tracker = utils::ProgressTracker::new(1, Some("region discovery"));
            let regions = cloud_interface.fetch_regions(&regions_tracker).await?;
            regions_tracker.finish_with_message(&format!(
                "Region discovery complete: found {} regions in {}",
                regions.len(),
                provider_id
            ));
            regions
        }
    };

    let mut total_instance_types = 0;
    let multi = utils::ProgressTracker::create_multi();
    let main_tracker = utils::ProgressTracker::add_to_multi(
        &multi,
        regions.len() as u64,
        Some("regions processed"),
    );
    for (index, region) in regions.iter().enumerate() {
        main_tracker.set_position(index as u64);
        main_tracker.update_message(&format!(
            "Processing instances in region '{}' ({}/{})",
            region,
            index + 1,
            regions.len(),
        ));

        // Fetch instance types from the provider
        let instances_tracker = utils::ProgressTracker::new_indeterminate(
            &multi,
            &format!("Fetching '{}' instance type details...", region),
        );
        let instance_types = cloud_interface
            .fetch_instance_types(region, &instances_tracker)
            .await?;
        let instance_types_count = instance_types.len();

        InstanceType::upsert_many(pool, instance_types).await?;

        // Increment the main tracker after completing a region
        total_instance_types += instance_types_count;
        main_tracker.inc(1);
        main_tracker.update_message(&format!(
            "Completed region {}/{}: {}",
            index + 1,
            regions.len(),
            region
        ));
    }

    main_tracker.finish_with_message(&format!(
        "Instance type discovery complete: found {} instances in {}",
        total_instance_types, provider_id
    ));

    println!("Instance type loading completed for '{}'", provider_id);

    Ok(())
}
