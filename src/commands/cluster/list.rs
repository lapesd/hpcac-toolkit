use crate::database::models::Cluster;
use sqlx::sqlite::SqlitePool;
use tabled::{Table, Tabled, settings::Style};

#[derive(Tabled)]
struct ClusterDisplay {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Cluster Name")]
    display_name: String,
    #[tabled(rename = "Nodes")]
    node_count: usize,
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<()> {
    let clusters = Cluster::fetch_all(pool).await?;
    let total = clusters.len();

    if clusters.is_empty() {
        println!("\nNo Clusters found.");
    } else {
        let mut table_rows: Vec<ClusterDisplay> = vec![];
        for cluster in clusters {
            let nodes = cluster.get_nodes(pool).await?;
            let node_count = nodes.len();
            table_rows.push(ClusterDisplay {
                id: cluster.id,
                provider: cluster.provider_id,
                display_name: cluster.display_name,
                node_count,
            })
        }

        let mut table = Table::new(table_rows);
        table.with(Style::rounded());
        println!("\nClusters:");
        println!("{}", table);
        println!("Found {} Clusters.", total);
    }

    Ok(())
}
