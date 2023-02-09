use std::process::exit;

use colored::Colorize;
use prettytable::{row, Table};
use sqlx::PgPool;

use hpcac_core::models::aws_cluster_schematic::AwsClusterSchematic;

pub async fn action(pool: &PgPool) {
    match AwsClusterSchematic::fetch_all(pool).await {
        Ok(schematics) => {
            let mut pretty_table = Table::new();
            pretty_table.set_titles(row![
                b=> "Alias",
                "Description",
                "Worker Count",
                "Worker Flavor",
                "Spot"
            ]);
            for schematic in schematics {
                pretty_table.add_row(row![
                    schematic.alias,
                    schematic.description,
                    schematic.worker_count.to_string(),
                    schematic.workers_flavor,
                    schematic.spot_cluster.to_string()
                ]);
            }
            println!("{}", "Saved AWS Cluster Schemas:".green().bold());
            pretty_table.printstd();
        }
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };
}
