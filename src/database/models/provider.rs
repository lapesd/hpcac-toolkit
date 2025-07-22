use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use tracing::error;

#[derive(Deserialize, Serialize, Debug)]
pub struct Provider {
    pub id: String,
    pub display_name: String,
    pub required_variables: String,
    pub supports_spot: bool,
}

impl Provider {
    pub fn get_required_config_vars(&self) -> Vec<String> {
        self.required_variables
            .split(',')
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty())
            .collect()
    }

    pub async fn fetch_by_id(pool: &SqlitePool, id: String) -> Result<Option<Provider>> {
        let provider = match sqlx::query_as!(
            Provider,
            r#"
                SELECT 
                    id as "id!", 
                    display_name,
                    required_variables,
                    supports_spot 
                FROM providers
                WHERE id = ?
            "#,
            id
        )
        .fetch_optional(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                bail!("DB Operation Failure");
            }
        };
        Ok(provider)
    }

    pub async fn fetch_all(pool: &SqlitePool) -> Result<Vec<Provider>> {
        let providers = match sqlx::query_as!(
            Provider,
            r#"
                SELECT 
                    id as "id!", 
                    display_name,
                    required_variables,
                    supports_spot 
                FROM providers
            "#
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                bail!("DB Operation Failure");
            }
        };
        Ok(providers)
    }
}
