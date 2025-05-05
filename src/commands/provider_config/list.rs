use crate::database::models::ProviderConfig;
use sqlx::sqlite::SqlitePool;
use tabled::{Table, Tabled, settings::Style};

#[derive(Tabled)]
struct ProviderConfigDisplay {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Config Name")]
    display_name: String,
    #[tabled(rename = "Config Variables")]
    config_vars: String,
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<()> {
    let configs = ProviderConfig::fetch_all(pool).await?;
    let total = configs.len();

    if configs.is_empty() {
        println!("\nNo Provider Configurations found");
    } else {
        let mut table_rows: Vec<ProviderConfigDisplay> = vec![];
        for config in configs {
            let config_vars = config.get_config_vars(pool).await?;
            let config_vars_str = config_vars
                .iter()
                .map(|cv| cv.to_string())
                .collect::<Vec<String>>()
                .join(" | ");

            table_rows.push(ProviderConfigDisplay {
                id: config.id,
                provider: config.provider_id,
                display_name: config.display_name.clone(),
                config_vars: config_vars_str,
            })
        }

        let mut table = Table::new(table_rows);
        table.with(Style::rounded());
        println!("\nProviderConfigs:");
        println!("{}", table);
        println!("Found {} Provider Configurations", total);
    }

    Ok(())
}
