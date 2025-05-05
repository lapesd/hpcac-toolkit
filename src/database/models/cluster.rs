use crate::database::models::{Node, ShellCommand};
use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use tracing::{debug, error};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Cluster {
    pub id: String,
    pub display_name: String,
    pub provider_id: String,
    pub provider_config_id: i64,
    pub public_ssh_key_path: String,
    pub private_ssh_key_path: String,
    pub region: String,
    pub created_at: NaiveDateTime,
    pub spawned_at: Option<NaiveDateTime>,
}

impl Cluster {
    pub async fn fetch_by_id(pool: &SqlitePool, cluster_id: &str) -> Result<Cluster> {
        let cluster = match sqlx::query_as!(
            Cluster,
            r#"
                SELECT 
                    id as "id!", 
                    display_name,
                    provider_id,
                    provider_config_id as "provider_config_id!",
                    public_ssh_key_path,
                    private_ssh_key_path,
                    region,
                    created_at,
                    spawned_at
                FROM clusters
                WHERE id = ?
            "#,
            cluster_id
        )
        .fetch_one(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        Ok(cluster)
    }

    pub async fn fetch_all(pool: &SqlitePool) -> Result<Vec<Cluster>> {
        let clusters = match sqlx::query_as!(
            Cluster,
            r#"
                SELECT 
                    id as "id!", 
                    display_name,
                    provider_id,
                    provider_config_id as "provider_config_id!",
                    public_ssh_key_path,
                    private_ssh_key_path,
                    region,
                    created_at,
                    spawned_at
                FROM clusters
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

        Ok(clusters)
    }

    pub async fn insert(
        &self,
        pool: &SqlitePool,
        nodes: Vec<Node>,
        commands: Vec<ShellCommand>,
    ) -> Result<()> {
        debug!(
            "Starting cluster insertion transaction for cluster_id: {}",
            self.id
        );

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to begin transaction: {}", e.to_string());
                anyhow::bail!("DB Operation Failure");
            }
        };

        // Insert the cluster first using the transaction
        debug!("Inserting cluster with id: {}", self.id);
        match sqlx::query!(
            r#"
                INSERT INTO clusters (
                    id,
                    display_name, 
                    provider_id,
                    provider_config_id, 
                    public_ssh_key_path, 
                    private_ssh_key_path, 
                    region,
                    created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.display_name,
            self.provider_id,
            self.provider_config_id,
            self.public_ssh_key_path,
            self.private_ssh_key_path,
            self.region,
            self.created_at
        )
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {
                debug!("Successfully inserted cluster with id: {}", self.id);
            }
            Err(e) => {
                error!("Failed to insert cluster: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: Cluster insertion failed");
            }
        };

        debug!("Inserting {} nodes", nodes.len());
        for (i, node) in nodes.iter().enumerate() {
            debug!(
                "Inserting node {}/{} with id: {}, cluster_id: {}",
                i + 1,
                nodes.len(),
                node.id,
                node.cluster_id
            );

            if node.cluster_id != self.id {
                error!(
                    "Node {} has cluster_id {} but we're inserting cluster {}",
                    node.id, node.cluster_id, self.id
                );
                anyhow::bail!("Node cluster_id mismatch");
            }

            if let Err(e) = node.insert(&mut tx).await {
                error!("Failed to insert node {}: {}", node.id, e);
                return Err(e);
            }
        }

        debug!("Inserting {} commands", commands.len());
        for (i, command) in commands.iter().enumerate() {
            debug!(
                "Inserting command {}/{} for node_id: {}",
                i + 1,
                commands.len(),
                command.node_id
            );

            if !nodes.iter().any(|n| n.id == command.node_id) {
                error!(
                    "Command references node_id {} which is not in our nodes list",
                    command.node_id
                );
                anyhow::bail!("Command references non-existent node");
            }

            if let Err(e) = command.insert(&mut tx).await {
                error!("Failed to insert command: {}", e);
                return Err(e);
            }
        }

        debug!("Committing transaction");
        match tx.commit().await {
            Ok(_) => {
                debug!("Transaction committed successfully");
            }
            Err(e) => {
                error!("Failed to commit transaction: {}", e.to_string());
                anyhow::bail!("DB Operation Failure: Transaction commit failed");
            }
        };

        Ok(())
    }

    pub async fn get_nodes(&self, pool: &SqlitePool) -> Result<Vec<Node>> {
        let nodes = match sqlx::query_as!(
            Node,
            r#"
                SELECT
                    id as "id!", 
                    cluster_id, 
                    status, 
                    instance_type, 
                    allocation_mode, 
                    burstable_mode, 
                    image_id, 
                    private_ip, 
                    public_ip 
                FROM nodes 
                WHERE cluster_id = ?
            "#,
            self.id
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("{}", e.to_string());
                anyhow::bail!("DB Operation Failure")
            }
        };

        Ok(nodes)
    }
}
