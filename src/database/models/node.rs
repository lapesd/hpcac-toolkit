use crate::database::models::ShellCommand;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sqlx::{Transaction, sqlite::SqlitePool};
use tracing::error;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Node {
    pub id: String,
    pub cluster_id: String,
    pub instance_type: String,
    pub allocation_mode: String,
    pub burstable_mode: Option<String>,
    pub image_id: String,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub was_efs_configured: bool,
    pub was_ssh_configured: bool,
}

impl Node {
    pub async fn insert(&self, tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
        match sqlx::query!(
            r#"
                INSERT INTO nodes (
                    id,
                    cluster_id, 
                    instance_type, 
                    allocation_mode, 
                    burstable_mode, 
                    image_id,
                    was_efs_configured,
                    was_ssh_configured
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.cluster_id,
            self.instance_type,
            self.allocation_mode,
            self.burstable_mode,
            self.image_id,
            self.was_efs_configured,
            self.was_ssh_configured,
        )
        .execute(&mut **tx)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                bail!("DB Operation Failure");
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

    pub async fn set_efs_configuration_state(
        &self,
        pool: &SqlitePool,
        configured: bool,
    ) -> Result<()> {
        match sqlx::query!(
            r#"
                UPDATE nodes 
                SET was_efs_configured = ? 
                WHERE id = ?
            "#,
            configured,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    error!("No node found with id '{}'", self.id);
                    bail!("DB Operation Failure");
                }
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        }

        Ok(())
    }

    pub async fn set_ips(
        &self,
        pool: &SqlitePool,
        private_ip: &str,
        public_ip: &str,
    ) -> Result<()> {
        match sqlx::query!(
            r#"
            UPDATE nodes 
            SET private_ip = ?, public_ip = ? 
            WHERE id = ?
        "#,
            private_ip,
            public_ip,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    error!("No node found with id '{}'", self.id);
                    bail!("DB Operation Failure");
                }
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        }
        Ok(())
    }

    pub async fn fetch_by_private_ip(pool: &SqlitePool, private_ip: &str) -> Result<Option<Node>> {
        match sqlx::query_as!(
            Node,
            r#"
            SELECT 
                id as "id!",
                cluster_id,
                instance_type,
                allocation_mode,
                burstable_mode,
                image_id,
                private_ip,
                public_ip,
                was_efs_configured,
                was_ssh_configured
            FROM nodes 
            WHERE private_ip = ?
        "#,
            private_ip
        )
        .fetch_optional(pool)
        .await
        {
            Ok(node) => Ok(node),
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        }
    }
}
