use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};

#[derive(Deserialize, Serialize, FromRow)]
pub struct AwsMachineImage {
    pub code: String,
    pub alias: String,
}

impl AwsMachineImage {
    pub fn to_string(&self) -> String {
        format!("{}", self.code)
    }

    pub fn describe(&self) -> String {
        format!("{} | {}", self.code, self.alias,)
    }
}

impl AwsMachineImage {
    pub async fn fetch_all(pool: &PgPool) -> Result<Vec<AwsMachineImage>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_amis";
            "#,
        )
        .map(|row: PgRow| AwsMachineImage {
            code: row.get(0),
            alias: row.get(1),
        })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }
}
