use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};

#[derive(Deserialize, Serialize, FromRow)]
pub struct AwsAvailabilityZone {
    pub code: String,
}

impl AwsAvailabilityZone {
    pub async fn fetch_all(pool: &PgPool) -> Result<Vec<AwsAvailabilityZone>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_azs";
            "#,
        )
        .map(|row: PgRow| AwsAvailabilityZone { code: row.get(0) })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }
}
