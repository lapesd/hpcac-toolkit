use crate::database::models::{InstanceType, Node, ProviderConfig, ShellCommand};
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
    pub availability_zone: String,
    pub created_at: NaiveDateTime,
    pub spawned_at: Option<NaiveDateTime>,
    pub node_affinity: bool,
}

impl Cluster {
    pub async fn print_details(&self, pool: &SqlitePool) -> Result<()> {
        let provider_config =
            match ProviderConfig::fetch_by_id(pool, self.provider_config_id).await? {
                Some(config) => config,
                None => {
                    error!("Missing ProviderConfig '{}'", self.provider_config_id);
                    anyhow::bail!("Data Consistency Failure");
                }
            };

        let nodes = self.get_nodes(pool).await?;

        println!("\n=== Cluster '{}' ===", self.display_name);
        println!("{:<20}: {}", "Provider", self.provider_id);
        println!("{:<20}: {}", "Region", self.region);
        println!(
            "{:<20}: {}",
            "Provider Config", provider_config.display_name
        );
        println!("{:<20}: {}\n", "Node Count", nodes.len());

        println!("Node Details:");
        for (i, node) in nodes.iter().enumerate() {
            let instance_type_name = &node.instance_type;
            let instance_details = match InstanceType::fetch_by_name_and_region(
                pool,
                instance_type_name,
                &self.region,
            )
            .await?
            {
                Some(instance_type) => instance_type,
                None => {
                    error!("Missing InstanceType '{}'", instance_type_name);
                    anyhow::bail!("Data Consistency Failure");
                }
            };

            let processor_info = match &instance_details.core_count {
                Some(cores) => {
                    format!(
                        "{}-Core {} {}",
                        cores, instance_details.cpu_architecture, instance_details.cpu_type
                    )
                }
                None => {
                    format!(
                        "{} {}",
                        instance_details.cpu_architecture, instance_details.cpu_type
                    )
                }
            };

            let gpu_info = match instance_details.gpu_type {
                Some(gpu) => {
                    format!("{}x {}", instance_details.gpu_count, gpu)
                }
                None => "N/A".to_string(),
            };

            println!("  Node {}:", i + 1);
            println!("    Instance Type   : {}", node.instance_type);
            println!("    Processor       : {}", processor_info);
            println!("    vCPUs:          : {}", instance_details.vcpus);
            println!("    GPUs:           : {}", gpu_info);
            println!("    Image ID        : {}", node.image_id);
            println!("    Allocation Mode : {}", node.allocation_mode);
            println!(
                "    Burstable Mode  : {}",
                node.burstable_mode.as_deref().unwrap_or("N/A")
            );
            println!();
        }

        Ok(())
    }

    pub async fn fetch_by_id(pool: &SqlitePool, cluster_id: &str) -> Result<Option<Cluster>> {
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
                    availability_zone,
                    created_at,
                    spawned_at,
                    node_affinity
                FROM clusters
                WHERE id = ?
            "#,
            cluster_id
        )
        .fetch_optional(pool)
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
                    availability_zone,
                    created_at,
                    spawned_at,
                    node_affinity
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
                    availability_zone,
                    created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.display_name,
            self.provider_id,
            self.provider_config_id,
            self.public_ssh_key_path,
            self.private_ssh_key_path,
            self.region,
            self.availability_zone,
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
