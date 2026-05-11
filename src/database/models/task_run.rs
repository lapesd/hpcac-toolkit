use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePool;

#[derive(Debug, sqlx::FromRow)]
pub struct TaskRun {
    pub id: i64,
    pub cluster_id: String,
    pub task_name: String,
    pub run_index: i64,
    pub status: String,
    pub started_at: NaiveDateTime,
    pub finished_at: Option<NaiveDateTime>,
}

impl TaskRun {
    /// Insert a new task run record with status "running" and return it.
    pub async fn start(
        pool: &SqlitePool,
        cluster_id: &str,
        task_name: &str,
        run_index: i64,
    ) -> Result<Self> {
        let started_at = chrono::Utc::now().naive_utc();
        let id = match sqlx::query!(
            r#"
                INSERT INTO task_runs (cluster_id, task_name, run_index, status, started_at)
                VALUES (?, ?, ?, 'running', ?)
            "#,
            cluster_id,
            task_name,
            run_index,
            started_at,
        )
        .execute(pool)
        .await
        {
            Ok(result) => result.last_insert_rowid(),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        };

        Ok(Self {
            id,
            cluster_id: cluster_id.to_string(),
            task_name: task_name.to_string(),
            run_index,
            status: "running".to_string(),
            started_at,
            finished_at: None,
        })
    }

    /// Mark the task run as finished with the given status ("success" or "failed").
    pub async fn finish(&self, pool: &SqlitePool, status: &str) -> Result<()> {
        let finished_at = chrono::Utc::now().naive_utc();
        match sqlx::query!(
            r#"
                UPDATE task_runs
                SET status = ?, finished_at = ?
                WHERE id = ?
            "#,
            status,
            finished_at,
            self.id,
        )
        .execute(pool)
        .await
        {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
    }
}
