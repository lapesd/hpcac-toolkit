use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use std::fmt;
use tracing::error;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConfigVar {
    pub id: i64,
    pub provider_config_id: i64,
    pub key: String,
    pub value: String,
}

impl fmt::Display for ConfigVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let masked_suffix = if self.value.len() >= 4 {
            &self.value[self.value.len() - 4..]
        } else {
            &self.value[..]
        };

        write!(f, "{}: \"****{}\"", self.key, masked_suffix)
    }
}

pub trait ConfigVarFinder {
    /// Returns a reference to the ConfigVar with the given key, if found.
    fn get_var(&self, key: &str) -> Option<&ConfigVar>;

    /// Returns the value associated with the given key, if found.
    fn get_value(&self, key: &str) -> Option<&str>;
}

impl ConfigVarFinder for [ConfigVar] {
    fn get_var(&self, key: &str) -> Option<&ConfigVar> {
        self.iter().find(|cv| cv.key == key)
    }

    fn get_value(&self, key: &str) -> Option<&str> {
        self.get_var(key).map(|cv| cv.value.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderConfig {
    pub id: i64,
    pub provider_id: String,
    pub display_name: String,
}

impl ProviderConfig {
    pub async fn fetch_by_id(pool: &SqlitePool, id: i64) -> Result<Option<ProviderConfig>> {
        let config = match sqlx::query_as!(
            ProviderConfig,
            r#"
                SELECT
                    id, 
                    provider_id as "provider_id!", 
                    display_name as "display_name!" 
                FROM provider_configs
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
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(config)
    }

    pub async fn fetch_all_by_provider(
        pool: &SqlitePool,
        provider_id: &str,
    ) -> Result<Vec<ProviderConfig>> {
        let configs = match sqlx::query_as!(
            ProviderConfig,
            r#"
                SELECT
                    id, 
                    provider_id as "provider_id!", 
                    display_name as "display_name!" 
                FROM provider_configs
                WHERE provider_id = ?
            "#,
            provider_id
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(configs)
    }

    pub async fn fetch_all(pool: &SqlitePool) -> Result<Vec<ProviderConfig>> {
        let configs = match sqlx::query_as!(
            ProviderConfig,
            r#"
                SELECT 
                    id, 
                    provider_id as "provider_id!", 
                    display_name as "display_name!"
                FROM provider_configs
            "#,
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(configs)
    }

    pub async fn insert(
        pool: &SqlitePool,
        display_name: String,
        provider_id: String,
        mut config_vars: Vec<ConfigVar>,
    ) -> Result<()> {
        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };
        let inserted_config = match sqlx::query(
            r#"
                INSERT INTO provider_configs (provider_id, display_name)
                VALUES (?, ?)
                RETURNING *;
            "#,
        )
        .bind(provider_id.clone())
        .bind(display_name.clone())
        .map(|row: SqliteRow| ProviderConfig {
            id: row.get(0),
            provider_id: row.get(1),
            display_name: row.get(2),
        })
        .fetch_one(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        for config_var in config_vars.iter_mut() {
            let stmt = sqlx::query(
                r#"
                    INSERT INTO config_variables (provider_config_id, key, value)
                    VALUES (?, ?, ?)
                "#,
            )
            .bind(inserted_config.id)
            .bind(config_var.key.clone())
            .bind(config_var.value.clone());

            let _ = match stmt.execute(&mut *tx).await {
                Ok(result) => result,
                Err(e) => {
                    error!("SQLx Error: {}", e.to_string());
                    anyhow::bail!("DB Operation Failure");
                }
            };
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_config_vars(&self, pool: &SqlitePool) -> Result<Vec<ConfigVar>> {
        let config_vars = match sqlx::query_as!(
            ConfigVar,
            r#"
                SELECT id, provider_config_id, key, value FROM config_variables 
                WHERE provider_config_id = ?
            "#,
            self.id
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(config_vars)
    }

    pub async fn delete(&self, pool: &SqlitePool) -> Result<()> {
        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        // First delete associated config variables (foreign key constraint)
        let _ = match sqlx::query(
            r#"
                DELETE FROM config_variables
                WHERE provider_config_id = ?
            "#,
        )
        .bind(self.id)
        .execute(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        // Then delete the provider config itself
        let _ = match sqlx::query(
            r#"
                DELETE FROM provider_configs
                WHERE id = ?
            "#,
        )
        .bind(self.id)
        .execute(&mut *tx)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        tx.commit().await?;
        Ok(())
    }
}
