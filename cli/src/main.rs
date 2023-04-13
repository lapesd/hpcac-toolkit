use std::process::exit;

use clap::Command;
use colored::Colorize;
use sqlx::PgPool;

mod actions;
mod config;

fn cli() -> Command {
    Command::new("hpcac-cli")
        .about("Manage cloud-based virtual clusters and HPC workloads lifecycle")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .subcommand(
            Command::new("schematic")
                .about("Manage schematics, or cluster blueprints")
                .arg_required_else_help(true)
                .args_conflicts_with_subcommands(true)
                .subcommand(Command::new("add").about("Create a new cloud cluster schematic"))
                .subcommand(Command::new("ls").about("List cloud clusters schematics")),
        )
        .subcommand(
            Command::new("cluster")
                .about("Manage cloud clusters")
                .arg_required_else_help(true)
                .args_conflicts_with_subcommands(true)
                .subcommand(
                    Command::new("spawn")
                        .about("Create a new cloud cluster based on existing schematics"),
                ),
        )
}

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    let config = config::load_config_from_environment_variables();

    let pool = match PgPool::connect(&config.pg_uri).await {
        Ok(result) => result,
        Err(exception) => {
            println!(
                "{}: Postgres problem, {}.\n",
                "ERROR".red().bold(),
                exception
            );
            exit(0);
        }
    };

    match matches.subcommand() {
        Some(("schematic", sub_matches)) => match sub_matches.subcommand() {
            Some(("add", _sub_matches)) => {
                let provider = config::provider_selector(&pool).await;
                match provider.as_str() {
                    "aws" => actions::schematic::add_aws_schematic::action(&pool).await,
                    _ => unreachable!("Provider {:?} not defined!", provider),
                }
            }
            Some(("ls", _sub_matches)) => {
                let provider = config::provider_selector(&pool).await;
                match provider.as_str() {
                    "aws" => actions::schematic::ls_aws_schematic::action(&pool).await,
                    _ => unreachable!("Provider {:?} not defined!", provider),
                }
            }
            _ => {
                println!("{}: Subcommand not defined.\n", "ERROR".red().bold());
                exit(0);
            }
        },
        Some(("cluster", sub_matches)) => match sub_matches.subcommand() {
            Some(("spawn", _sub_matches)) => {
                let provider = config::provider_selector(&pool).await;
                match provider.as_str() {
                    "aws" => actions::cluster::spawn_aws_cluster::action(&pool).await,
                    _ => unreachable!("Provider {:?} not defined!", provider),
                }
            }
            _ => {
                println!("{}: Subcommand not defined.\n", "ERROR".red().bold());
                exit(0);
            }
        },
        _ => unreachable!("Command {:?} not defined!", matches),
    }
}
