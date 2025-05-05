use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::Transaction;
use sqlx::sqlite::SqlitePool;
use tracing::error;

#[derive(Deserialize, Serialize, Debug)]
pub struct ShellCommand {
    pub id: i64,
    pub ordering: i64,
    pub node_id: String,
    pub script: String,
    pub status: String,
    pub result: Option<String>,
    pub triggered_at: Option<NaiveDateTime>,
    pub execution_time: Option<i64>,
}

impl ShellCommand {
    pub async fn _fetch_all_by_node_id(
        pool: &SqlitePool,
        node_id: String,
    ) -> Result<Vec<ShellCommand>> {
        let rows = match sqlx::query_as!(
            ShellCommand,
            r#"
                SELECT 
                    id as "id!", 
                    ordering, 
                    node_id, 
                    script, 
                    status,
                    result, 
                    triggered_at,
                    execution_time
                FROM shell_commands
                WHERE node_id = ?
            "#,
            node_id
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

        Ok(rows)
    }

    pub async fn insert(&self, tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
        match sqlx::query!(
            r#"
                INSERT INTO shell_commands (
                    ordering, 
                    node_id, 
                    script, 
                    status
                )
                VALUES (?, ?, ?, ?)
            "#,
            self.ordering,
            self.node_id,
            self.script,
            self.status,
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
