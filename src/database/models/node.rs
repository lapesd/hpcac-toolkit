use crate::database::models::ShellCommand;

use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Transaction, sqlite::SqlitePool};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
/// A node slot in a cluster.
///
/// The fields fall into three categories, and confusing them is the source of
/// most state-drift bugs in this toolkit. Keep new fields in the right group.
///
/// 1. DESIRED STATE (the spec). What the user asked for: `instance_type`,
///    `allocation_mode`, `burstable_mode`, `image_id`, `root_volume_*`. Changed
///    only by the user or by the recovery policy. Never drifts, because it is
///    intent rather than a description of the world.
///
/// 2. OBSERVED STATE. What the provider last reported: `instance_id`,
///    `private_ip`, `public_ip`. These CAN drift: the instance may be gone, the
///    address reassigned. `observed_at` says when they were last confirmed;
///    treat them as a snapshot, not as truth, and prefer `instance_id` as the
///    identity of the machine, since the addresses are reused across slots and
///    cleared by `reset()`.
///
/// 3. STEPS COMPLETED. What this toolkit did: `was_efs_configured`. Only the
///    toolkit writes these, and they must be cleared whenever the underlying
///    machine is replaced, or a later step will skip work it still needs to do.
///    (`was_ssh_configured` is vestigial: never read, never set true.)
pub struct Node {
    pub id: String,
    pub cluster_id: String,
    pub instance_type: String,
    pub allocation_mode: String,
    pub burstable_mode: Option<String>,
    pub image_id: String,
    pub root_volume_gb: i64,
    pub root_volume_type: String,
    pub root_volume_iops: Option<i64>,
    pub instance_id: Option<String>,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub observed_at: Option<NaiveDateTime>,
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
                    root_volume_gb,
                    root_volume_type,
                    root_volume_iops,
                    was_efs_configured,
                    was_ssh_configured
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.cluster_id,
            self.instance_type,
            self.allocation_mode,
            self.burstable_mode,
            self.image_id,
            self.root_volume_gb,
            self.root_volume_type,
            self.root_volume_iops,
            self.was_efs_configured,
            self.was_ssh_configured,
        )
        .execute(&mut **tx)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: {}", e);
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

    pub async fn reset(&self, pool: &SqlitePool) -> Result<()> {
        match sqlx::query!(
            r#"UPDATE nodes SET private_ip = '', public_ip = NULL, was_efs_configured = false WHERE id = ?"#,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    anyhow::bail!("Node '{}' not found for reset", self.id);
                }
            }
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
        Ok(())
    }

    /// Look a node up by the provider's stable handle rather than by an address.
    /// Prefer this over `fetch_by_private_ip`: `private_ip` is an observation and
    /// may be empty (see `Node::reset`) or reassigned to a different machine.
    pub async fn fetch_by_instance_id(
        pool: &SqlitePool,
        cluster_id: &str,
        instance_id: &str,
    ) -> Result<Option<Node>> {
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
                root_volume_gb,
                root_volume_type,
                root_volume_iops,
                instance_id,
                private_ip,
                public_ip,
                observed_at,
                was_efs_configured,
                was_ssh_configured
            FROM nodes
            WHERE cluster_id = ? AND instance_id = ?
        "#,
            cluster_id,
            instance_id
        )
        .fetch_optional(pool)
        .await
        {
            Ok(node) => Ok(node),
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }
    }

    /// Record what the provider last reported for this node, stamping the time so
    /// readers can tell how stale the observation is. Only the observed-state
    /// fields are touched; the spec and the step flags are left alone.
    pub async fn record_observation(
        pool: &SqlitePool,
        node_id: &str,
        instance_id: Option<&str>,
        private_ip: Option<&str>,
        public_ip: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().naive_utc();
        match sqlx::query!(
            r#"UPDATE nodes
               SET instance_id = COALESCE(?, instance_id),
                   private_ip  = COALESCE(?, private_ip),
                   public_ip   = COALESCE(?, public_ip),
                   observed_at = ?
               WHERE id = ?"#,
            instance_id,
            private_ip,
            public_ip,
            now,
            node_id
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
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
                    anyhow::bail!("Node '{}' not found for EFS configuration update", self.id);
                }
            }
            Err(e) => {
                anyhow::bail!("DB Operation Failure: {}", e);
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
                    anyhow::bail!("Node '{}' not found for IP update", self.id);
                }
            }
            Err(e) => {
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }
        Ok(())
    }

    /// Updates instance_type and allocation_mode in-place when recovery policy
    /// specifies a different spec for this node slot.
    /// Overwrites the hardware spec of an existing node row.
    ///
    /// A replacement node reuses the failed node's row, so every field the
    /// recovery policy can override has to be written here. Updating only the
    /// instance type silently produces mismatched nodes: a `g7e` slot that keeps
    /// the r8i's CPU image boots a GPU instance with no CUDA on it, and the
    /// mismatch is invisible until the application fails to find a device.
    pub async fn update_instance_spec(
        &self,
        pool: &SqlitePool,
        instance_type: &str,
        allocation_mode: &str,
        image_id: &str,
        burstable_mode: Option<&str>,
        root_volume_gb: i64,
        root_volume_type: &str,
        root_volume_iops: Option<i64>,
    ) -> Result<()> {
        match sqlx::query!(
            r#"UPDATE nodes SET
                   instance_type = ?,
                   allocation_mode = ?,
                   image_id = ?,
                   burstable_mode = ?,
                   root_volume_gb = ?,
                   root_volume_type = ?,
                   root_volume_iops = ?
               WHERE id = ?"#,
            instance_type,
            allocation_mode,
            image_id,
            burstable_mode,
            root_volume_gb,
            root_volume_type,
            root_volume_iops,
            self.id,
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    anyhow::bail!("Node '{}' not found for instance spec update", self.id);
                }
            }
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
        Ok(())
    }

    /// Removes a node from a cluster by its private address.
    ///
    /// Scoped to the cluster for the same reason as `fetch_by_private_ip`, and
    /// more urgently: unscoped, this deletes the matching row in EVERY cluster
    /// that has ever used that address, destroying rows for clusters that are
    /// not even part of the operation.
    pub async fn delete_by_private_ip(
        pool: &SqlitePool,
        cluster_id: &str,
        private_ip: &str,
    ) -> Result<()> {
        let result = sqlx::query!(
            r#"DELETE FROM nodes WHERE cluster_id = ? AND private_ip = ?"#,
            cluster_id,
            private_ip
        )
        .execute(pool)
        .await
        .map_err(|e| anyhow::anyhow!("DB Operation Failure: {}", e))?;

        if result.rows_affected() == 0 {
            tracing::warn!(
                "No node with private_ip='{}' in cluster '{}' to delete",
                private_ip,
                cluster_id
            );
        }
        Ok(())
    }

    /// Looks a node up by its private address WITHIN a cluster.
    ///
    /// The cluster_id is not optional. Every cluster this toolkit provisions uses
    /// the same private range, so 10.0.0.11 exists in as many clusters as have
    /// been spawned, and a lookup on the address alone silently returns whichever
    /// row the query planner reaches first. That is how a two-node recovery came
    /// to skip mounting the shared filesystem: the "mark this node for EFS
    /// re-configuration" step cleared the flag on a stale row belonging to a
    /// different, already-terminated cluster, so the live node kept its flag and
    /// the mount was skipped without any error.
    pub async fn fetch_by_private_ip(
        pool: &SqlitePool,
        cluster_id: &str,
        private_ip: &str,
    ) -> Result<Option<Node>> {
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
                root_volume_gb,
                root_volume_type,
                root_volume_iops,
                instance_id,
                private_ip,
                public_ip,
                observed_at,
                was_efs_configured,
                was_ssh_configured
            FROM nodes
            WHERE cluster_id = ? AND private_ip = ?
        "#,
            cluster_id,
            private_ip
        )
        .fetch_optional(pool)
        .await
        {
            Ok(node) => Ok(node),
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }
    }
}
