use crate::database::models::{InstanceType, InstanceTypeFilters};
use sqlx::sqlite::SqlitePool;
use tabled::{Table, Tabled, settings::Style};

#[derive(Tabled)]
struct InstanceTypeDisplay {
    #[tabled(rename = "Provider")]
    provider: String,
    #[tabled(rename = "Region")]
    region: String,
    #[tabled(rename = "Instance Type")]
    name: String,
    #[tabled(rename = "Is Baremetal")]
    is_baremetal: bool,
    #[tabled(rename = "Price per Hour")]
    price_per_hour: f64,
    #[tabled(rename = "Price per Core")]
    price_per_core: f64,
    #[tabled(rename = "Cores")]
    cores: i64,
    #[tabled(rename = "Processor")]
    processor: String,
    #[tabled(rename = "GPUs")]
    gpus: String,
    #[tabled(rename = "FPGAs")]
    fpgas: String,
    #[tabled(rename = "Memory")]
    memory: i64,
    #[tabled(rename = "Spot Support")]
    supports_spot: bool,
    #[tabled(rename = "Affinity Option")]
    has_affinity_settings: bool,
    #[tabled(rename = "Supports EFA")]
    supports_efa: bool,
}

pub async fn list(pool: &SqlitePool, filters: InstanceTypeFilters) -> anyhow::Result<()> {
    let instance_types = InstanceType::fetch_all(pool, filters).await?;
    let total = instance_types.len();

    if instance_types.is_empty() {
        println!("\nNo Instance Types found");
    } else {
        let mut table_rows: Vec<InstanceTypeDisplay> = vec![];
        for instance_type in instance_types {
            let processor = format!(
                "{} {}",
                instance_type.cpu_type, instance_type.cpu_architecture,
            );
            let gpus = match instance_type.gpu_type {
                Some(gpu_model) => format!("{}x {}", instance_type.gpu_count, gpu_model),
                None => "0".to_string(),
            };
            let fpgas = match instance_type.fpga_type {
                Some(fpga_model) => format!("{}x {}", instance_type.fpga_count, fpga_model),
                None => "0".to_string(),
            };
            let cores = instance_type.core_count.unwrap_or_else(|| {
                instance_type.vcpus / instance_type.threads_per_core.unwrap_or(1)
            });
            let price_per_hour = instance_type.on_demand_price_per_hour.unwrap_or(0f64);
            let price_per_core = (price_per_hour / cores as f64 * 10000.0).trunc() / 10000.0;

            table_rows.push(InstanceTypeDisplay {
                provider: instance_type.provider_id,
                region: instance_type.region,
                name: instance_type.name,
                is_baremetal: instance_type.is_baremetal,
                price_per_hour,
                cores,
                price_per_core,
                processor,
                gpus,
                fpgas,
                memory: instance_type.memory_in_mib,
                supports_spot: instance_type.supports_spot,
                has_affinity_settings: instance_type.has_affinity_settings,
                supports_efa: instance_type.supports_efa,
            })
        }

        table_rows.sort_by(|a, b| {
            a.price_per_core
                .partial_cmp(&b.price_per_core)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut table = Table::new(table_rows);
        table.with(Style::rounded());
        println!("\nInstance Types:");
        println!("{}", table);
        println!("Found {} Instance Types", total);
    }

    Ok(())
}
