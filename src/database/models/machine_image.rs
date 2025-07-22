use anyhow::{Result, bail};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MachineImage {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub creation_date: String,
    pub provider: String,
    pub region: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl MachineImage {
    pub async fn _fetch_by_id(pool: &SqlitePool, image_id: &str) -> Result<MachineImage> {
        let image = match sqlx::query_as!(
            MachineImage,
            r#"
                SELECT 
                    id as "id!", 
                    name as "name!",
                    description as "description!",
                    owner as "owner!",
                    creation_date as "creation_date!",
                    provider as "provider!",
                    region as "region!",
                    created_at as "created_at!",
                    updated_at as "updated_at!"
                FROM machine_images
                WHERE id = ?
            "#,
            image_id
        )
        .fetch_one(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(image)
    }

    pub async fn _fetch_by_provider_region(
        pool: &SqlitePool,
        provider: &str,
        region: &str,
    ) -> Result<Vec<MachineImage>> {
        let images = match sqlx::query_as!(
            MachineImage,
            r#"
                SELECT 
                    id as "id!", 
                    name as "name!",
                    description as "description!",
                    owner as "owner!",
                    creation_date as "creation_date!",
                    provider as "provider!",
                    region as "region!",
                    created_at as "created_at!",
                    updated_at as "updated_at!"
                FROM machine_images
                WHERE provider = ? AND region = ?
            "#,
            provider,
            region
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(images)
    }

    pub async fn _insert(&self, pool: &SqlitePool) -> Result<()> {
        match sqlx::query!(
            r#"
            INSERT INTO machine_images (
                id,
                name,
                description,
                owner,
                creation_date,
                provider,
                region,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
            self.id,
            self.name,
            self.description,
            self.owner,
            self.creation_date,
            self.provider,
            self.region,
            self.created_at,
            self.updated_at
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to insert machine image: {}", e.to_string());
                bail!("DB Operation Failure: {}", e);
            }
        }
    }
}
