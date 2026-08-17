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
    /// Init commands for the replacement, as a JSON array. NULL means "inherit":
    /// an in-place replacement keeps whatever is already attached to its node row,
    /// and scale-up nodes copy the slot they fan out from. Set this when the
    /// replacement needs different preparation from the node it replaces — a GPU
    /// stand-in for a CPU node has a local NVMe store to mount that the original
    /// never had.
    pub init_commands: Option<String>,
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

    /// Init commands declared for this slot, in order. `None` means the slot
    /// declares none and the inherit behaviour applies; `Some(vec![])` means an
    /// explicitly empty list, which clears the replacement's commands.
    pub fn declared_init_commands(&self) -> Option<Vec<String>> {
        let raw = self.init_commands.as_deref()?;
        match serde_json::from_str::<Vec<String>>(raw) {
            Ok(commands) => Some(commands),
            Err(e) => {
                tracing::warn!(
                    "Recovery node '{}' has unparseable init_commands ({}), ignoring: {}",
                    self.id,
                    e,
                    raw
                );
                None
            }
        }
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
                    count,
                    init_commands
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            self.init_commands,
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
                    count,
                    init_commands
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
