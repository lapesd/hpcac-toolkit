use anyhow::{Error, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};
use uuid::Uuid;

#[derive(Deserialize, Serialize, FromRow)]
pub struct Provider {
    pub uuid: Uuid,
    pub alias: String,
    pub added_at: NaiveDateTime,
    pub schematics_table: String,
}

impl Provider {
    pub async fn fetch_all(pool: &PgPool) -> Result<Vec<Provider>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."providers";
            "#,
        )
        .map(|row: PgRow| Provider {
            uuid: row.get(0),
            alias: row.get(1),
            added_at: row.get(2),
            schematics_table: row.get(3),
        })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }
}
