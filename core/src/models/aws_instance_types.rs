use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::types::Decimal;
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};
use uuid::Uuid;

#[derive(Deserialize, Serialize, FromRow)]
pub struct AwsMachineType {
    pub uuid: Uuid,
    pub instance_type: String,
    pub instance_size: String,
    pub vcpus: i32,
    pub memory: f64,
    pub on_demand_linux_pricing: Decimal,
}

impl AwsMachineType {
    pub fn to_string(&self) -> String {
        format!("{}.{}", self.instance_type, self.instance_size)
    }

    pub fn describe(&self) -> String {
        format!(
            "{}.{} | {} vCPUs | {}GB RAM | {} USD/h",
            self.instance_type,
            self.instance_size,
            self.vcpus,
            self.memory,
            self.on_demand_linux_pricing
        )
    }
}

impl AwsMachineType {
    pub async fn fetch_all(pool: &PgPool) -> Result<Vec<AwsMachineType>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_instance_types";
            "#,
        )
        .map(|row: PgRow| AwsMachineType {
            uuid: row.get(0),
            instance_type: row.get(1),
            instance_size: row.get(2),
            vcpus: row.get(3),
            memory: row.get(4),
            on_demand_linux_pricing: row.get(5),
        })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }

    pub async fn filter_by_minimum_resources(
        pool: &PgPool,
        vcpus: i32,
        memory: f64,
    ) -> Result<Vec<AwsMachineType>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_instance_types"
                WHERE vcpus >= $1 AND memory >= $2;
            "#,
        )
        .bind(vcpus)
        .bind(memory)
        .map(|row: PgRow| AwsMachineType {
            uuid: row.get(0),
            instance_type: row.get(1),
            instance_size: row.get(2),
            vcpus: row.get(3),
            memory: row.get(4),
            on_demand_linux_pricing: row.get(5),
        })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }
}
