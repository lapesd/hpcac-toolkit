use crate::database::models::{InstanceType, Node, ProviderConfig, ShellCommand};

use anyhow::Result;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Type, sqlite::SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Default)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "snake_case")]
pub enum ClusterState {
    #[default]
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "spawning")]
    Spawning,
    #[sqlx(rename = "running")]
    Running,
    #[sqlx(rename = "restoring")]
    Restoring,
    #[sqlx(rename = "terminating")]
    Terminating,
    #[sqlx(rename = "terminated")]
    Terminated,
    #[sqlx(rename = "failed")]
    Failed,
}

impl std::fmt::Display for ClusterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            ClusterState::Pending => "pending",
            ClusterState::Spawning => "spawning",
            ClusterState::Running => "running",
            ClusterState::Restoring => "restoring",
            ClusterState::Terminating => "terminating",
            ClusterState::Terminated => "terminated",
            ClusterState::Failed => "failed",
        };
        write!(f, "{}", state_str)
    }
}

impl std::str::FromStr for ClusterState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ClusterState::Pending),
            "spawning" => Ok(ClusterState::Spawning),
            "running" => Ok(ClusterState::Running),
            "restoring" => Ok(ClusterState::Restoring),
            "terminating" => Ok(ClusterState::Terminating),
            "terminated" => Ok(ClusterState::Terminated),
            "failed" => Ok(ClusterState::Failed),
            _ => Err(format!("Invalid cluster state: '{}'", s)),
        }
    }
}

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
    pub use_node_affinity: bool,
    pub use_elastic_fabric_adapters: bool,
    pub use_elastic_file_system: bool,
    pub created_at: NaiveDateTime,
    pub state: ClusterState,
}

impl Cluster {
    pub async fn print_details(&self, pool: &SqlitePool) -> Result<()> {
        let provider_config =
            match ProviderConfig::fetch_by_id(pool, self.provider_config_id).await? {
                Some(config) => config,
                None => {
                    tracing::error!("Missing ProviderConfig '{}'", self.provider_config_id);
                    anyhow::bail!("Data Consistency Failure");
                }
            };

        let nodes = self.get_nodes(pool).await?;

        tracing::info!(
            "\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n{:<35}: {}\n\nNode Details:",
            "Cluster Name",
            self.display_name,
            "Provider",
            self.provider_id,
            "Region",
            self.region,
            "Availability Zone",
            self.availability_zone,
            "Use Node Affinity",
            self.use_node_affinity,
            "Use Elastic Fabric Adapters (EFAs)",
            self.use_elastic_fabric_adapters,
            "Use Elastic File System (EFS)",
            self.use_elastic_file_system,
            "Provider Config",
            provider_config.display_name,
            "Node Count",
            nodes.len()
        );
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
                    tracing::error!("Missing InstanceType '{}'", instance_type_name);
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

            tracing::info!(
                "  Node {}:\n    Instance Type   : {}\n    Processor       : {}\n    vCPUs:          : {}\n    GPUs:           : {}\n    Image ID        : {}\n    Allocation Mode : {}\n    Burstable Mode  : {}",
                i + 1,
                node.instance_type,
                processor_info,
                instance_details.vcpus,
                gpu_info,
                node.image_id,
                node.allocation_mode,
                node.burstable_mode.as_deref().unwrap_or("N/A")
            );
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
                    use_node_affinity,
                    use_elastic_fabric_adapters,
                    use_elastic_file_system,
                    created_at,
                    state as "state: ClusterState"
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
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(cluster)
    }

    pub async fn fetch_by_name(pool: &SqlitePool, cluster_name: &str) -> Result<Option<Cluster>> {
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
                    use_node_affinity,
                    use_elastic_fabric_adapters,
                    use_elastic_file_system,
                    created_at,
                    state as "state: ClusterState"
                FROM clusters
                WHERE display_name = ?
            "#,
            cluster_name
        )
        .fetch_optional(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
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
                    use_node_affinity,
                    use_elastic_fabric_adapters,
                    use_elastic_file_system,
                    created_at,
                    state as "state: ClusterState"
                FROM clusters
            "#,
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
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
        tracing::info!(
            "Starting cluster insertion transaction for cluster_id: {}",
            self.id
        );

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        tracing::info!("Inserting Cluster (id='{}')", self.id);
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
                    use_node_affinity,
                    use_elastic_fabric_adapters,
                    use_elastic_file_system,
                    created_at,
                    state
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            self.id,
            self.display_name,
            self.provider_id,
            self.provider_config_id,
            self.public_ssh_key_path,
            self.private_ssh_key_path,
            self.region,
            self.availability_zone,
            self.use_node_affinity,
            self.use_elastic_fabric_adapters,
            self.use_elastic_file_system,
            self.created_at,
            self.state,
        )
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {
                tracing::info!("Successfully inserted cluster with id: {}", self.id);
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        tracing::info!("Inserting {} Nodes", nodes.len());
        for (i, node) in nodes.iter().enumerate() {
            tracing::info!(
                "Inserting Node (id='{}') {} of {} for Cluster (id='{}')",
                node.id,
                i + 1,
                nodes.len(),
                node.cluster_id,
            );

            if node.cluster_id != self.id {
                tracing::error!(
                    "Node (id='{}') has cluster_id '{}' but we're inserting Cluster (id='{}')",
                    node.id,
                    node.cluster_id,
                    self.id
                );
                anyhow::bail!(
                    "Node '{}' does not belong to cluster '{}'",
                    node.id,
                    self.id
                );
            }

            node.insert(&mut tx).await?;
        }

        tracing::info!("Inserting {} Commands", commands.len());
        for (i, command) in commands.iter().enumerate() {
            tracing::info!(
                "Inserting Command {} of {} for Node: (id='{}')",
                i + 1,
                commands.len(),
                command.node_id
            );

            if !nodes.iter().any(|n| n.id == command.node_id) {
                tracing::error!(
                    "Command references Node (id='{}') which is not in our nodes list",
                    command.node_id
                );
                anyhow::bail!("Command references unknown node '{}'", command.node_id);
            }

            command.insert(&mut tx).await?;
        }

        tracing::info!("Committing transaction");
        match tx.commit().await {
            Ok(_) => {
                tracing::info!("Transaction committed successfully");
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, cluster_id: &str) -> Result<()> {
        tracing::info!("Starting deletion of Cluster (id='{}')", cluster_id);

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // First, delete all commands associated with nodes in this cluster
        tracing::info!("Deleting commands for Cluster (id='{}')", cluster_id);
        match sqlx::query!(
            r#"
                DELETE FROM shell_commands 
                WHERE node_id IN (
                    SELECT id FROM nodes WHERE cluster_id = ?
                )
            "#,
            cluster_id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                tracing::info!(
                    "Deleted {} commands for Cluster (id='{}')",
                    result.rows_affected(),
                    cluster_id
                );
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // Then, delete all nodes associated with this cluster
        tracing::info!("Deleting nodes for Cluster (id='{}')", cluster_id);
        match sqlx::query!(
            r#"
                DELETE FROM nodes 
                WHERE cluster_id = ?
            "#,
            cluster_id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                tracing::info!(
                    "Deleted {} nodes for Cluster (id='{}')",
                    result.rows_affected(),
                    cluster_id
                );
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // Finally, delete the cluster itself
        tracing::info!("Deleting Cluster (id='{}')", cluster_id);
        match sqlx::query!(
            r#"
                DELETE FROM clusters 
                WHERE id = ?
            "#,
            cluster_id
        )
        .execute(&mut *tx)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    tracing::warn!("No cluster found with id '{}' for deletion", cluster_id);
                    anyhow::bail!("Cluster not found");
                }
                tracing::info!("Successfully deleted Cluster (id='{}')", cluster_id);
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        // Commit the transaction
        tracing::info!(
            "Committing deletion transaction for Cluster (id='{}')",
            cluster_id
        );
        match tx.commit().await {
            Ok(_) => {
                tracing::info!(
                    "Successfully deleted Cluster (id='{}') and all associated data",
                    cluster_id
                );
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
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
                    instance_type, 
                    allocation_mode, 
                    burstable_mode, 
                    image_id, 
                    private_ip, 
                    public_ip,
                    was_efs_configured,
                    was_ssh_configured
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
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        };

        Ok(nodes)
    }

    pub async fn update_state(&self, pool: &SqlitePool, new_state: ClusterState) -> Result<()> {
        tracing::info!(
            "Transitioning Cluster (id='{}') to state '{}'",
            self.id,
            new_state
        );

        match sqlx::query!(
            r#"
                UPDATE clusters 
                SET state = ? 
                WHERE id = ?
            "#,
            new_state,
            self.id
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    anyhow::bail!("Cluster '{}' not found for state transition", self.id);
                }

                tracing::info!(
                    "Successfully transitioned Cluster (id='{}') to '{}'",
                    self.id,
                    new_state
                );
            }
            Err(e) => {
                tracing::error!("SQLx Error: {:?}", e);
                anyhow::bail!("DB Operation Failure: {}", e);
            }
        }

        Ok(())
    }
}
