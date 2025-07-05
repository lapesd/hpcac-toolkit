use crate::database::models::{InstanceType, Node, ProviderConfig, ShellCommand, InstanceCreationFailurePolicy};


use anyhow::{Result, bail};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{Type, sqlite::SqlitePool};
use tracing::{error, info, warn};

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
            "terminating" => Ok(ClusterState::Terminating),
            "terminated" => Ok(ClusterState::Terminated),
            "failed" => Ok(ClusterState::Failed),
            _ => Err(format!("Invalid cluster state: '{}'", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub on_instance_creation_failure: Option<InstanceCreationFailurePolicy>,
    pub migration_attempts: i64,
    pub tried_zones: Option<String>,
}

impl Cluster {
    pub async fn print_details(&self, pool: &SqlitePool) -> Result<()> {
        let provider_config =
            match ProviderConfig::fetch_by_id(pool, self.provider_config_id).await? {
                Some(config) => config,
                None => {
                    error!("Missing ProviderConfig '{}'", self.provider_config_id);
                    bail!("Data Consistency Failure");
                }
            };

        let nodes = self.get_nodes(pool).await?;

        println!("\n{:<35}: {}", "Cluster Name", self.display_name);
        println!("{:<35}: {}", "Provider", self.provider_id);
        println!("{:<35}: {}", "Region", self.region);
        println!("{:<35}: {}", "Availability Zone", self.availability_zone);
        println!("{:<35}: {}", "Use Node Affinity", self.use_node_affinity);
        println!(
            "{:<35}: {}",
            "Use Elastic Fabric Adapters (EFAs)", self.use_elastic_fabric_adapters
        );
        println!(
            "{:<35}: {}",
            "Use Elastic File System (EFS)", self.use_elastic_file_system
        );
        println!(
            "{:<35}: {}",
            "On Instance Creation Failure", self.on_instance_creation_failure.clone().unwrap_or(InstanceCreationFailurePolicy::Cancel).to_string()
        );
        println!(
            "{:<35}: {}",
            "Provider Config", provider_config.display_name
        );
        println!("{:<35}: {}\n", "Node Count", nodes.len());

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
                    bail!("Data Consistency Failure");
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
                    use_node_affinity,
                    use_elastic_fabric_adapters,
                    use_elastic_file_system,
                    created_at,
                    state as "state: ClusterState",
                    on_instance_creation_failure as "on_instance_creation_failure: InstanceCreationFailurePolicy",
                    migration_attempts as "migration_attempts!",
                    tried_zones 
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
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
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
                    state as "state: ClusterState",
                    on_instance_creation_failure as "on_instance_creation_failure: InstanceCreationFailurePolicy",
                    migration_attempts as "migration_attempts!",
                    tried_zones
                FROM clusters
            "#,
        )
        .fetch_all(pool)
        .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
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
        info!(
            "Starting cluster insertion transaction for cluster_id: {}",
            self.id
        );

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        info!("Inserting Cluster (id='{}')", self.id);
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
                    state,
                    on_instance_creation_failure
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            self.on_instance_creation_failure,
        )
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {
                info!("Successfully inserted cluster with id: {}", self.id);
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        info!("Inserting {} Nodes", nodes.len());
        for (i, node) in nodes.iter().enumerate() {
            info!(
                "Inserting Node (id='{}') {} of {} for Cluster (id='{}')",
                node.id,
                i + 1,
                nodes.len(),
                node.cluster_id,
            );

            if node.cluster_id != self.id {
                error!(
                    "Node (id='{}') has cluster_id '{}' but we're inserting Cluster (id='{}')",
                    node.id, node.cluster_id, self.id
                );
                bail!("DB Operation Failure");
            }

            node.insert(&mut tx).await?;
        }

        info!("Inserting {} Commands", commands.len());
        for (i, command) in commands.iter().enumerate() {
            info!(
                "Inserting Command {} of {} for Node: (id='{}')",
                i + 1,
                commands.len(),
                command.node_id
            );

            if !nodes.iter().any(|n| n.id == command.node_id) {
                error!(
                    "Command references Node (id='{}') which is not in our nodes list",
                    command.node_id
                );
                bail!("DB Operation Failure");
            }

            command.insert(&mut tx).await?;
        }

        info!("Committing transaction");
        match tx.commit().await {
            Ok(_) => {
                info!("Transaction committed successfully");
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, cluster_id: &str) -> Result<()> {
        info!("Starting deletion of Cluster (id='{}')", cluster_id);

        let mut tx = match pool.begin().await {
            Ok(result) => result,
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        // First, delete all commands associated with nodes in this cluster
        info!("Deleting commands for Cluster (id='{}')", cluster_id);
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
                info!(
                    "Deleted {} commands for Cluster (id='{}')",
                    result.rows_affected(),
                    cluster_id
                );
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        // Then, delete all nodes associated with this cluster
        info!("Deleting nodes for Cluster (id='{}')", cluster_id);
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
                info!(
                    "Deleted {} nodes for Cluster (id='{}')",
                    result.rows_affected(),
                    cluster_id
                );
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        // Finally, delete the cluster itself
        info!("Deleting Cluster (id='{}')", cluster_id);
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
                    warn!("No cluster found with id '{}' for deletion", cluster_id);
                    bail!("Cluster not found");
                }
                info!("Successfully deleted Cluster (id='{}')", cluster_id);
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        // Commit the transaction
        info!(
            "Committing deletion transaction for Cluster (id='{}')",
            cluster_id
        );
        match tx.commit().await {
            Ok(_) => {
                info!(
                    "Successfully deleted Cluster (id='{}') and all associated data",
                    cluster_id
                );
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
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
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        };

        Ok(nodes)
    }

    pub async fn update_cluster_state(
        pool: &SqlitePool,
        cluster_id: &str,
        new_state: ClusterState,
    ) -> Result<()> {
        info!(
            "Transitioning Cluster (id='{}') to state '{}'",
            cluster_id, new_state
        );

        match sqlx::query!(
            r#"
                UPDATE clusters 
                SET state = ? 
                WHERE id = ?
            "#,
            new_state,
            cluster_id
        )
        .execute(pool)
        .await
        {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    error!(
                        "No cluster found with id '{}' for state transition",
                        cluster_id
                    );
                    bail!("DB Operation Failure");
                }

                info!(
                    "Successfully transitioned Cluster (id='{}') to '{}'",
                    cluster_id, new_state
                );
            }
            Err(e) => {
                error!("SQLx Error: {:?}", e);
                bail!("DB Operation Failure");
            }
        }

        Ok(())
    }
}
