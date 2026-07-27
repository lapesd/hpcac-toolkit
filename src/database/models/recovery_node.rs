use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Transaction, sqlite::SqlitePool};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct RecoveryNode {
    pub id: String,
    pub cluster_id: String,
    pub allocation_mode: String,
    pub preferred_instance_types: String, // JSON array
    pub burstable_mode: Option<String>,
    pub image_id: String,
    pub root_volume_gb: i64,
    pub root_volume_type: String,
    pub root_volume_iops: Option<i64>,
    /// Number of nodes to provision for this slot on recovery.
    /// Default 1 preserves same-size (1:1) replacement. `count > 1` enables scale-up:
    /// the failed slot's original node is respawned with this spec, and
    /// (count - 1) additional Node rows are created in the DB with the same spec.
    pub count: i64,
}

impl RecoveryNode {
    /// Returns the preferred instance types in priority order.
    pub fn instance_types(&self) -> Vec<String> {
        serde_json::from_str(&self.preferred_instance_types).unwrap_or_default()
    }

    /// Returns the highest-priority instance type.
    pub fn primary_instance_type(&self) -> Option<String> {
        self.instance_types().into_iter().next()
    }

    pub async fn insert(&self, tx: &mut Transaction<'_, sqlx::Sqlite>) -> Result<()> {
        match sqlx::query!(
            r#"
                INSERT INTO recovery_nodes (
                    id,
                    cluster_id,
                    allocation_mode,
                    preferred_instance_types,
                    burstable_mode,
                    image_id,
                    root_volume_gb,
                    root_volume_type,
                    root_volume_iops,
                    count
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.cluster_id,
            self.allocation_mode,
            self.preferred_instance_types,
            self.burstable_mode,
            self.image_id,
            self.root_volume_gb,
            self.root_volume_type,
            self.root_volume_iops,
            self.count,
        )
        .execute(&mut **tx)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("SQLx Error: {}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }
        Ok(())
    }

    /// Returns recovery nodes for a cluster in insertion order (slot 0, 1, 2, …).
    pub async fn fetch_all_by_cluster_id(
        pool: &SqlitePool,
        cluster_id: &str,
    ) -> Result<Vec<RecoveryNode>> {
        match sqlx::query_as!(
            RecoveryNode,
            r#"
                SELECT
                    id as "id!",
                    cluster_id,
                    allocation_mode,
                    preferred_instance_types,
                    burstable_mode,
                    image_id,
                    root_volume_gb,
                    root_volume_type,
                    root_volume_iops,
                    count
                FROM recovery_nodes
                WHERE cluster_id = ?
            "#,
            cluster_id
        )
        .fetch_all(pool)
        .await
        {
            Ok(rows) => Ok(rows),
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }
    }
}
