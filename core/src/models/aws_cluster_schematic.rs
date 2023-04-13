use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, PgPool, Row};
use uuid::Uuid;

#[derive(Deserialize, Serialize, FromRow)]
pub struct AwsClusterSchematic {
    pub uuid: Uuid,
    pub alias: String,
    pub description: String,
    pub az: String,
    pub master_ami: String,
    pub master_flavor: String,
    pub master_ebs: i32,
    pub spot_cluster: bool,
    pub worker_count: i32,
    pub workers_ami: String,
    pub workers_flavor: String,
    pub workers_ebs: i32,
    pub nfs_support: bool,
    pub criu_support: bool,
    pub blcr_support: bool,
    pub ulfm_support: bool,
}

impl AwsClusterSchematic {
    pub async fn generate_hcl_files(self) {}

    pub async fn insert(self, pool: &PgPool) -> Result<(), Error> {
        let _inserted_submission = sqlx::query(
            r#"
                INSERT INTO "public"."aws_clusters_schematics"
                (
                    uuid, alias, description, az, master_ami, 
                    master_flavor, master_ebs, spot_cluster, worker_count, workers_ami, 
                    workers_flavor, workers_ebs, nfs_support, criu_support, 
                    blcr_support, ulfm_support
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16);
            "#,
        )
        .bind(&self.uuid)
        .bind(&self.alias)
        .bind(&self.description)
        .bind(&self.az)
        .bind(&self.master_ami)
        .bind(&self.master_flavor)
        .bind(&self.master_ebs)
        .bind(&self.spot_cluster)
        .bind(&self.worker_count)
        .bind(&self.workers_ami)
        .bind(&self.workers_flavor)
        .bind(&self.workers_ebs)
        .bind(&self.nfs_support)
        .bind(&self.criu_support)
        .bind(&self.blcr_support)
        .bind(&self.ulfm_support)
        .execute(&*pool)
        .await?;
        Ok(())
    }

    pub async fn fetch_all(pool: &PgPool) -> Result<Vec<AwsClusterSchematic>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_clusters_schematics";
            "#,
        )
        .map(|row: PgRow| AwsClusterSchematic {
            uuid: row.get(0),
            alias: row.get(1),
            description: row.get(2),
            az: row.get(3),
            master_ami: row.get(4),
            master_flavor: row.get(5),
            master_ebs: row.get(6),
            spot_cluster: row.get(7),
            worker_count: row.get(8),
            workers_ami: row.get(9),
            workers_flavor: row.get(10),
            workers_ebs: row.get(11),
            nfs_support: row.get(12),
            criu_support: row.get(13),
            blcr_support: row.get(14),
            ulfm_support: row.get(15),
        })
        .fetch_all(&*pool)
        .await?;
        Ok(record)
    }

    pub async fn fetch_by_alias(
        alias: &str,
        pool: &PgPool,
    ) -> Result<Option<AwsClusterSchematic>, Error> {
        let record = sqlx::query(
            r#"
                SELECT * FROM "public"."aws_clusters_schematics"
                WHERE alias = $1;
            "#,
        )
        .bind(alias)
        .map(|row: PgRow| AwsClusterSchematic {
            uuid: row.get(0),
            alias: row.get(1),
            description: row.get(2),
            az: row.get(3),
            master_ami: row.get(4),
            master_flavor: row.get(5),
            master_ebs: row.get(6),
            spot_cluster: row.get(7),
            worker_count: row.get(8),
            workers_ami: row.get(9),
            workers_flavor: row.get(10),
            workers_ebs: row.get(11),
            nfs_support: row.get(12),
            criu_support: row.get(13),
            blcr_support: row.get(14),
            ulfm_support: row.get(15),
        })
        .fetch_optional(&*pool)
        .await?;
        Ok(record)
    }
}
