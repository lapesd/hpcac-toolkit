use anyhow::{Result, bail};
use chrono::Utc;
use clap::{Parser, Subcommand};
use sqlx::sqlite::SqlitePool;
use std::fs::OpenOptions;
use tracing::{error, info};

mod commands;
mod constants;
mod database;
mod integrations;
mod utils;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Cluster management commands
    Cluster {
        #[command(subcommand)]
        command: ClusterCommands,
    },

    /// Instance type management commands
    InstanceType {
        #[command(subcommand)]
        command: InstanceTypeCommands,
    },

    /// Provider configuration management commands
    ProviderConfig {
        #[command(subcommand)]
        command: ProviderConfigCommands,
    },
}

#[derive(Subcommand, Debug)]
enum ClusterCommands {
    /// Create a Cluster
    Create {
        /// Path to the YAML file with cluster details
        #[arg(short = 'f', long = "file")]
        yaml_file_path: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Delete a Cluster
    Delete {
        /// Identifier of the Cluster to delete
        #[arg(long)]
        cluster_id: String,
    },

    /// List existing Clusters
    List {},

    /// Spawn a new Cluster
    Spawn {
        /// Cluster identifier
        #[arg(long)]
        cluster_id: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Terminates a new Cluster
    Terminate {
        /// Cluster identifier
        #[arg(long)]
        cluster_id: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Test a Cluster failure
    TestFailure {
        /// Cluster identifier
        #[arg(long)]
        cluster_id: String,

        /// Node private_ip to terminate
        #[arg(long)]
        node_private_ip: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum InstanceTypeCommands {
    /// Fetchs available instance types
    List {
        /// Filter instance_types by provider (examples: 'aws', 'vultr')
        #[arg(long)]
        provider: Option<String>,

        /// Filter instance_types by region (example: 'us-east-1')
        #[arg(long)]
        region: Option<String>,

        /// Filter by instance_types by processor architecture (e.g., arm64, x86_64)
        #[arg(long)]
        architecture: Option<String>,

        /// Filter instance_types by maximum core count
        #[arg(long)]
        max_cores: Option<i64>,

        /// Filter instance_types by minimum core count
        #[arg(long)]
        min_cores: Option<i64>,

        /// Filter instance_types with GPUs
        #[arg(long)]
        with_gpu: Option<bool>,

        /// Filter instance_types with FPGAs
        #[arg(long)]
        with_fpga: Option<bool>,

        /// Filter instance_types by baremetal infrastructure
        #[arg(long)]
        baremetal: Option<bool>,

        /// Filter instance_types by spot allocation support
        #[arg(long)]
        spot: Option<bool>,

        /// Filter instance_types by burstable support
        #[arg(long)]
        burstable: Option<bool>,

        /// Filter instance_types by elastic fabric adapter support
        #[arg(long)]
        fabric_adapter: Option<bool>,
    },

    /// Load instance type data from providers
    Load {
        /// Load instance_types from provider (examples: 'aws', 'vultr')
        #[arg(long)]
        provider: Option<String>,

        /// Load instance_types using the defined provider config
        #[arg(long)]
        provider_config_id: Option<String>,

        /// Load instance_types from region (example: 'us-east-1')
        #[arg(long)]
        region: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ProviderConfigCommands {
    /// Create a new provider configuration
    Create {
        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// Delete existing provider configurations
    Delete {
        /// The ID of the provider configuration to delete
        #[arg(required = true)]
        id: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// List existing provider configurations
    List {},
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Read environment variables
    dotenvy::dotenv().ok();

    // Setup logger, file directory and tracing subscriber
    let logs_directory = match std::env::var("LOGS_DIRECTORY") {
        Ok(result) => result,
        Err(_) => {
            println!("LOGS_DIRECTORY environment variable not set, using default.");
            "./logs".to_string()
        }
    };

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!(
            "{}/{}.log",
            logs_directory,
            Utc::now().format("%Y-%m-%d_%H-%M-%S")
        ))
        .expect("Failed to open log file");

    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();

    // Read SQLite connection data from environment variables.
    // If DATABASE_URL is not set, default to a local SQLite database.
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(result) => result,
        Err(_) => {
            println!("DATABASE_URL environment variable not set, using default.");
            "sqlite://db.sqlite".to_string()
        }
    };

    // Create a SQLite connection pool
    let sqlite_pool = match SqlitePool::connect(&db_url).await {
        Ok(result) => result,
        Err(e) => {
            error!("{:?}", e);
            bail!("Couldn't connect to SQLite database");
        }
    };

    // Match clap commands and pass the SQLite pool to the command handlers
    info!("Invoked command: {:?}", cli.command);
    match &cli.command {
        Commands::Cluster { command } => match command {
            ClusterCommands::Create {
                yaml_file_path,
                yes,
            } => {
                commands::cluster::create(&sqlite_pool, yaml_file_path, *yes).await?;
            }
            ClusterCommands::Delete { cluster_id } => {
                commands::cluster::delete(&sqlite_pool, cluster_id).await?;
            }
            ClusterCommands::List {} => {
                commands::cluster::list(&sqlite_pool).await?;
            }
            ClusterCommands::Spawn { cluster_id, yes } => {
                commands::cluster::spawn(&sqlite_pool, cluster_id, *yes).await?;
            }
            ClusterCommands::Terminate { cluster_id, yes } => {
                commands::cluster::terminate(&sqlite_pool, cluster_id, *yes).await?;
            }
            ClusterCommands::TestFailure {
                cluster_id,
                node_private_ip,
                yes,
            } => {
                commands::cluster::test_failure(&sqlite_pool, cluster_id, node_private_ip, *yes)
                    .await?;
            }
        },
        Commands::InstanceType { command } => match command {
            InstanceTypeCommands::Load {
                provider,
                provider_config_id,
                region,
            } => {
                commands::instance_type::load(
                    &sqlite_pool,
                    provider.clone(),
                    provider_config_id.clone(),
                    region.clone(),
                )
                .await?;
            }
            InstanceTypeCommands::List {
                provider,
                region,
                architecture,
                max_cores,
                min_cores,
                with_gpu,
                with_fpga,
                baremetal,
                spot,
                burstable,
                fabric_adapter,
            } => {
                commands::instance_type::list(
                    &sqlite_pool,
                    database::models::InstanceTypeFilters {
                        provider: provider.clone(),
                        region: region.clone(),
                        architecture: architecture.clone(),
                        max_cores: *max_cores,
                        min_cores: *min_cores,
                        with_gpu: *with_gpu,
                        with_fpga: *with_fpga,
                        baremetal: *baremetal,
                        spot: *spot,
                        burstable: *burstable,
                        fabric_adapter: *fabric_adapter,
                    },
                )
                .await?;
            }
        },
        Commands::ProviderConfig { command } => match command {
            ProviderConfigCommands::Create { yes } => {
                commands::provider_config::create(&sqlite_pool, *yes).await?;
            }
            ProviderConfigCommands::Delete { id, yes } => {
                commands::provider_config::delete(&sqlite_pool, id, *yes).await?;
            }
            ProviderConfigCommands::List {} => {
                commands::provider_config::list(&sqlite_pool).await?;
            }
        },
    }

    Ok(())
}
