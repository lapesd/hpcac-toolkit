use crate::database::models::{Cluster, ProviderConfig};

use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tabled::{Table, Tabled, settings::Style};
use tracing::warn;

#[derive(Tabled)]
struct ClusterDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Saved At")]
    created_at: String,
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Credentials")]
    credentials: String,
    #[tabled(rename = "Cluster Name")]
    display_name: String,
    #[tabled(rename = "Node Count")]
    node_count: usize,
    #[tabled(rename = "State")]
    state: String,
}

pub async fn list(pool: &SqlitePool) -> Result<()> {
    let clusters = Cluster::fetch_all(pool).await?;

    if clusters.is_empty() {
        println!("\nNo Clusters found.");
        return Ok(());
    }

    let mut table_rows: Vec<ClusterDisplay> = vec![];
    for cluster in clusters {
        let provider_config_name =
            match ProviderConfig::fetch_by_id(pool, cluster.provider_config_id).await? {
                Some(config) => config.display_name,
                None => {
                    warn!(
                        "Cluster '{}' is missing it's ProviderConfig (id='{}')",
                        cluster.display_name, cluster.provider_config_id
                    );
                    "<< ERROR, CHECK LOGS >>".to_string()
                }
            };
        let nodes = cluster.get_nodes(pool).await?;
        let node_count = nodes.len();
        table_rows.push(ClusterDisplay {
            id: cluster.id,
            created_at: cluster.created_at.to_string(),
            provider: cluster.provider_id,
            credentials: provider_config_name,
            display_name: cluster.display_name,
            node_count,
            state: cluster.state.to_string(),
        })
    }

    let mut table = Table::new(table_rows);
    table.with(Style::rounded());
    println!("\nClusters:");
    println!("{}", table);
    Ok(())
}
