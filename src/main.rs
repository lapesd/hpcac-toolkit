use clap::{Parser, Subcommand};
use sqlx::sqlite::SqlitePool;
use std::process;
use tracing::{error, info, warn};

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

    /// ClusterBlueprint management commands
    ClusterBlueprint {
        #[command(subcommand)]
        command: ClusterBlueprintCommands,
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
    /// Spawn a new Cluster
    Spawn {
        /// Name of the Cluster to spawn
        #[arg(long)]
        cluster_id: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ClusterBlueprintCommands {
    /// Create a ClusterBlueprint
    Create {
        /// Path to the YAML file with cluster blueprint details
        #[arg(short = 'f', long = "file")]
        yaml_file_path: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long = "yes")]
        yes: bool,
    },

    /// List existing ClusterBlueprints
    List {},
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
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // Read SQLite connection data from environment variables.
    // If DATABASE_URL is not set, default to a local SQLite database.
    let db_url = match std::env::var("DATABASE_URL") {
        Ok(result) => result,
        Err(_) => {
            warn!("DATABASE_URL environment variable not set, using default.");
            String::from("sqlite://db.sqlite")
        }
    };

    // Create a SQLite connection pool
    let sqlite_pool = match SqlitePool::connect(&db_url).await {
        Ok(result) => {
            info!("SQLite connection pool created!");
            result
        }
        Err(error) => {
            error!("Couldn't connect to SQLite: {}", error);
            process::exit(1);
        }
    };

    // Match clap commands and pass the SQLite pool to the command handlers
    match &cli.command {
        Commands::Cluster { command } => match command {
            ClusterCommands::Spawn { cluster_id, yes } => {
                commands::cluster::spawn(&sqlite_pool, cluster_id, *yes).await?;
            }
        },
        Commands::ClusterBlueprint { command } => match command {
            ClusterBlueprintCommands::Create {
                yaml_file_path,
                yes,
            } => {
                commands::cluster_blueprint::create(&sqlite_pool, yaml_file_path, *yes).await?;
            }
            ClusterBlueprintCommands::List {} => {
                commands::cluster_blueprint::list(&sqlite_pool).await?;
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
