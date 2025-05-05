use crate::constants::SQLITE_BATCH_SIZE;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Arguments, FromRow, SqlitePool};
use tracing::error;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct InstanceType {
    pub name: String,
    pub cpu_architecture: String,
    pub vcpus: i64,
    pub core_count: Option<i64>,
    pub threads_per_core: Option<i64>,
    pub cpu_type: String,
    pub gpu_count: i64,
    pub gpu_type: Option<String>,
    pub fpga_count: i64,
    pub fpga_type: Option<String>,
    pub memory_in_mib: i64,
    pub supports_spot: bool,
    pub is_baremetal: bool,
    pub is_burstable: bool,
    pub supports_efa: bool,
    pub has_affinity_settings: bool,
    pub on_demand_price_per_hour: Option<f64>,
    pub spot_price_per_hour: Option<f64>,
    pub region: String,
    pub provider_id: String,
}

pub struct InstanceTypeFilters {
    pub provider: Option<String>,
    pub region: Option<String>,
    pub architecture: Option<String>,
    pub max_cores: Option<i64>,
    pub min_cores: Option<i64>,
    pub with_gpu: Option<bool>,
    pub with_fpga: Option<bool>,
    pub baremetal: Option<bool>,
    pub spot: Option<bool>,
}

impl InstanceType {
    pub async fn fetch_by_name_and_region(
        pool: &SqlitePool,
        name: &str,
        region: &str,
    ) -> Result<Option<InstanceType>> {
        sqlx::query_as!(
            InstanceType,
            r#"
                SELECT 
                    name, 
                    cpu_architecture, 
                    vcpus, 
                    core_count, 
                    threads_per_core, 
                    cpu_type, 
                    gpu_count, 
                    gpu_type, 
                    fpga_count, 
                    fpga_type, 
                    memory_in_mib, 
                    supports_spot, 
                    is_baremetal, 
                    is_burstable, 
                    supports_efa, 
                    has_affinity_settings, 
                    on_demand_price_per_hour, 
                    spot_price_per_hour, 
                    region,
                    provider_id
                FROM instance_types
                WHERE name = ? and region = ?
            "#,
            name,
            region
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("SQLx Error: {}", e.to_string());
            anyhow::anyhow!("DB Operation Failure")
        })
    }

    pub async fn fetch_all(
        pool: &SqlitePool,
        filters: InstanceTypeFilters,
    ) -> Result<Vec<InstanceType>> {
        let mut query = String::from(
            "SELECT 
            name, 
            cpu_architecture, 
            vcpus, 
            core_count, 
            threads_per_core, 
            cpu_type, 
            gpu_count, 
            gpu_type, 
            fpga_count, 
            fpga_type, 
            memory_in_mib, 
            supports_spot, 
            is_baremetal, 
            is_burstable, 
            supports_efa, 
            has_affinity_settings, 
            on_demand_price_per_hour, 
            spot_price_per_hour, 
            region,
            provider_id
        FROM instance_types",
        );

        let mut conditions = Vec::new();
        let mut args = sqlx::sqlite::SqliteArguments::default();

        if let Some(provider_id) = filters.provider {
            conditions.push("provider_id = ?");
            let _ = args.add(provider_id);
        }

        if let Some(region_filter) = filters.region {
            conditions.push("region = ?");
            let _ = args.add(region_filter);
        }

        if let Some(arch) = filters.architecture {
            conditions.push("cpu_architecture = ?");
            let _ = args.add(arch);
        }

        if let Some(max) = filters.max_cores {
            conditions.push("(core_count IS NULL OR core_count <= ?)");
            let _ = args.add(max);
        }

        if let Some(min) = filters.min_cores {
            conditions.push("(core_count IS NOT NULL AND core_count >= ?)");
            let _ = args.add(min);
        }

        if let Some(true) = filters.with_gpu {
            conditions.push("gpu_count > 0");
        }

        if let Some(true) = filters.with_fpga {
            conditions.push("fpga_count > 0");
        }

        if let Some(is_baremetal) = filters.baremetal {
            conditions.push("is_baremetal = ?");
            let _ = args.add(is_baremetal);
        }

        if let Some(supports_spot) = filters.spot {
            conditions.push("supports_spot = ?");
            let _ = args.add(supports_spot);
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        sqlx::query_as_with::<_, InstanceType, _>(&query, args)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                error!("SQLx Error: {}", e.to_string());
                anyhow::anyhow!("DB Operation Failure")
            })
    }

    pub async fn upsert_many(pool: &SqlitePool, instances: Vec<InstanceType>) -> Result<()> {
        for chunk in instances.chunks(SQLITE_BATCH_SIZE) {
            let mut tx = pool.begin().await.map_err(|e| {
                error!("SQLx Error: {}", e.to_string());
                anyhow::anyhow!("DB Operation Failure")
            })?;

            for instance in chunk {
                sqlx::query!(
                    r#"
            INSERT INTO instance_types (
                name, 
                cpu_architecture, 
                vcpus, 
                core_count, 
                threads_per_core, 
                cpu_type, 
                gpu_count, 
                gpu_type, 
                fpga_count, 
                fpga_type, 
                memory_in_mib, 
                supports_spot, 
                is_baremetal, 
                is_burstable, 
                supports_efa, 
                has_affinity_settings, 
                on_demand_price_per_hour, 
                spot_price_per_hour, 
                region,
                provider_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(name, region) DO UPDATE SET
                cpu_architecture = excluded.cpu_architecture,
                vcpus = excluded.vcpus,
                core_count = excluded.core_count,
                threads_per_core = excluded.threads_per_core,
                cpu_type = excluded.cpu_type,
                gpu_count = excluded.gpu_count,
                gpu_type = excluded.gpu_type,
                fpga_count = excluded.fpga_count,
                fpga_type = excluded.fpga_type,
                memory_in_mib = excluded.memory_in_mib,
                supports_spot = excluded.supports_spot,
                is_baremetal = excluded.is_baremetal,
                is_burstable = excluded.is_burstable,
                supports_efa = excluded.supports_efa,
                has_affinity_settings = excluded.has_affinity_settings,
                on_demand_price_per_hour = excluded.on_demand_price_per_hour,
                spot_price_per_hour = excluded.spot_price_per_hour,
                region = excluded.region,
                provider_id = excluded.provider_id
            "#,
                    instance.name,
                    instance.cpu_architecture,
                    instance.vcpus,
                    instance.core_count,
                    instance.threads_per_core,
                    instance.cpu_type,
                    instance.gpu_count,
                    instance.gpu_type,
                    instance.fpga_count,
                    instance.fpga_type,
                    instance.memory_in_mib,
                    instance.supports_spot,
                    instance.is_baremetal,
                    instance.is_burstable,
                    instance.supports_efa,
                    instance.has_affinity_settings,
                    instance.on_demand_price_per_hour,
                    instance.spot_price_per_hour,
                    instance.region,
                    instance.provider_id
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("SQLx Error: {}", e.to_string());
                    anyhow::anyhow!("DB Operation Failure")
                })?;
            }

            tx.commit().await.map_err(|e| {
                error!("SQLx Error: {}", e.to_string());
                anyhow::anyhow!("DB Operation Failure")
            })?;
        }

        Ok(())
    }
}
