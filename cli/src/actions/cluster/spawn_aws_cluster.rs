use std::process::exit;

use colored::Colorize;
use inquire::{Select, Text};
use sqlx::PgPool;

use hpcac_core::models::aws_cluster_schematic::AwsClusterSchematic;

use crate::actions::schematic;

async fn prompt_for_aws_cluster_schematic(pool: &PgPool) -> String {
    let aws_schematic_options = match AwsClusterSchematic::fetch_all(pool).await {
        Ok(options) => options
            .iter()
            .map(|schematic| schematic.alias.to_owned())
            .collect(),
        Err(exception) => {
            println!("{}: {}.\n", "ERROR".red().bold(), exception);
            exit(0);
        }
    };
    loop {
        match Select::new(
            "Select the desired cluster schematic:",
            aws_schematic_options,
        )
        .prompt()
        {
            Ok(schematic) => return schematic,
            Err(exception) => {
                println!("{}: {}.\n", "ERROR".red().bold(), exception);
                exit(0);
            }
        }
    }
}

pub async fn action(pool: &PgPool) {
    let schematic_alias = prompt_for_aws_cluster_schematic(pool).await;
    let schematic = AwsClusterSchematic::fetch_by_alias(&schematic_alias, pool).await;
}
