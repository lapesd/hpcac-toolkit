use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::Transaction;
use tracing::error;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Node {
    pub id: String,
    pub cluster_id: String,
    pub status: String,
    pub instance_type: String,
    pub allocation_mode: String,
    pub burstable_mode: Option<String>,
    pub image_id: String,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
}

impl Node {
    pub async fn insert(&self, tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
        match sqlx::query!(
            r#"
                INSERT INTO nodes (
                    id,
                    cluster_id, 
                    status, 
                    instance_type, 
                    allocation_mode, 
                    burstable_mode, 
                    image_id
                )
                VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.cluster_id,
            self.status,
            self.instance_type,
            self.allocation_mode,
            self.burstable_mode,
            self.image_id,
        )
        .execute(&mut **tx)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(())
    }
}
