use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;
use std::fmt;
use tracing::{debug, error};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResourceStatus {
    Creating,
    Running,
    Updating,
    Stopping,
    Stopped,
    Deleting,
    Error,
    NotFound,
    Unknown,
}

impl ResourceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceStatus::Creating => "creating",
            ResourceStatus::Running => "running",
            ResourceStatus::Updating => "updating",
            ResourceStatus::Stopping => "stopping",
            ResourceStatus::Stopped => "stopped",
            ResourceStatus::Deleting => "deleting",
            ResourceStatus::Error => "error",
            ResourceStatus::NotFound => "not_found",
            ResourceStatus::Unknown => "unknown",
        }
    }

    pub fn _from_string(status: &str) -> Self {
        match status.to_lowercase().as_str() {
            "creating" => ResourceStatus::Creating,
            "running" | "active" => ResourceStatus::Running,
            "updating" => ResourceStatus::Updating,
            "stopping" => ResourceStatus::Stopping,
            "stopped" | "inactive" => ResourceStatus::Stopped,
            "deleting" => ResourceStatus::Deleting,
            "error" | "failed" => ResourceStatus::Error,
            "not_found" => ResourceStatus::NotFound,
            _ => ResourceStatus::Unknown,
        }
    }
}

impl fmt::Display for ResourceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CloudResource {
    pub id: String,
    pub cluster_id: String,
    pub resource_type: String,
    pub provider: String,
    pub region: String,
    pub status: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl CloudResource {
    pub async fn _fetch_by_id(pool: &SqlitePool, resource_id: &str) -> Result<CloudResource> {
        let resource = match sqlx::query_as!(
            CloudResource,
            r#"
                SELECT 
                    id as "id!", 
                    cluster_id,
                    resource_type,
                    provider,
                    region,
                    status,
                    created_at,
                    updated_at
                FROM cloud_resources
                WHERE id = ?
            "#,
            resource_id
        )
        .fetch_one(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(resource)
    }

    pub async fn _fetch_by_cluster_id(
        pool: &SqlitePool,
        cluster_id: &str,
    ) -> Result<Vec<CloudResource>> {
        let resources = match sqlx::query_as!(
            CloudResource,
            r#"
                SELECT 
                    id as "id!", 
                    cluster_id,
                    resource_type,
                    provider,
                    region,
                    status,
                    created_at,
                    updated_at
                FROM cloud_resources
                WHERE cluster_id = ?
            "#,
            cluster_id
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(resources)
    }

    pub async fn _fetch_with_tags(
        &self,
        pool: &SqlitePool,
    ) -> Result<(CloudResource, HashMap<String, String>)> {
        let tags = match sqlx::query!(
            r#"
                SELECT key, value
                FROM resource_tags
                WHERE resource_id = ?
            "#,
            self.id
        )
        .fetch_all(pool)
        .await
        {
            Ok(rows) => {
                let mut tag_map = HashMap::new();
                for row in rows {
                    tag_map.insert(row.key, row.value);
                }
                tag_map
            }
            Err(e) => {
                error!("SQLx Error fetching tags: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok((self.clone(), tags))
    }

    pub async fn _insert(
        &self,
        pool: &SqlitePool,
        tags: Option<HashMap<String, String>>,
    ) -> Result<()> {
        debug!(
            "Inserting cloud resource: {} for cluster: {}",
            self.id, self.cluster_id
        );

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to begin transaction: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // Insert the resource
        match sqlx::query!(
            r#"
                INSERT INTO cloud_resources (
                    id,
                    cluster_id,
                    resource_type,
                    provider,
                    region,
                    status,
                    created_at,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.cluster_id,
            self.resource_type,
            self.provider,
            self.region,
            self.status,
            self.created_at,
            self.updated_at
        )
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {
                debug!("Successfully inserted cloud resource with id: {}", self.id);
            }
            Err(e) => {
                error!("Failed to insert cloud resource: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // Insert the tags if provided
        if let Some(tag_map) = tags {
            for (key, value) in tag_map {
                match sqlx::query!(
                    r#"
                        INSERT INTO resource_tags (
                            resource_id,
                            key,
                            value
                        )
                        VALUES (?, ?, ?)
                    "#,
                    self.id,
                    key,
                    value
                )
                .execute(&mut *tx)
                .await
                {
                    Ok(_) => {
                        debug!("Inserted tag {}={} for resource {}", key, value, self.id);
                    }
                    Err(e) => {
                        error!("Failed to insert tag: {}", e.to_string());
                        anyhow::bail!("DB Operation Failure: {}", e);
                    }
                };
            }
        }

        match tx.commit().await {
            Ok(_) => {
                debug!("Transaction committed successfully");
            }
            Err(e) => {
                error!("Failed to commit transaction: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(())
    }

    pub async fn _update_status(
        &self,
        pool: &SqlitePool,
        new_status: ResourceStatus,
    ) -> Result<()> {
        debug!(
            "Updating status of resource {} to {:?}",
            self.id, new_status
        );

        let status_str = new_status.to_string();

        match sqlx::query!(
            r#"
                UPDATE cloud_resources
                SET status = ?, updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
            "#,
            status_str,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(_) => {
                debug!("Successfully updated status of resource {}", self.id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to update resource status: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e)
            }
        }
    }

    pub async fn _delete(&self, pool: &SqlitePool) -> Result<()> {
        debug!("Deleting cloud resource: {}", self.id);

        // Due to ON DELETE CASCADE in the schema, deleting the resource
        // will automatically delete associated tags
        match sqlx::query!(
            r#"
                DELETE FROM cloud_resources
                WHERE id = ?
            "#,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(_) => {
                debug!("Successfully deleted cloud resource {}", self.id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to delete cloud resource: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e)
            }
        }
    }
}
