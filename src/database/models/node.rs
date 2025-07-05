use crate::database::models::ShellCommand;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Transaction, sqlite::SqlitePool};
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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

    pub async fn get_init_commands(&self, pool: &SqlitePool) -> Result<Vec<String>> {
        let shell_commands = ShellCommand::fetch_all_by_node_id(pool, self.id.clone()).await?;

        let mut sorted_shell_command_structs = shell_commands;
        sorted_shell_command_structs.sort_by_key(|command_struct| command_struct.ordering);

        let scripts: Vec<String> = sorted_shell_command_structs
            .into_iter()
            .map(|command_struct| command_struct.script)
            .collect();

        Ok(scripts)
    }
}
