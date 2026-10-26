use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePool;

/// A row in `task_runs`: one execution of one task on one cluster.
///
/// `start` inserts a row and returns a handle carrying only the id, since that is
/// all `finish` needs to close it. `fetch_recent` reads rows back in full for
/// `task list`.
#[derive(Debug)]
pub struct TaskRun {
    pub id: i64,
}

/// A completed or in-flight run, as read back from the database.
#[derive(Debug)]
pub struct TaskRunRecord {
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

        Ok(Self { id })
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

    /// Close any run on this cluster still marked 'running', giving it `status`.
    ///
    /// A run only reaches `finish` when the task loop completes normally. Three
    /// paths bypass it: the watch loop aborting an in-flight run when a node
    /// fails, Ctrl+C cancelling the loop, and the controller being killed. This
    /// is how those rows get closed, and the caller supplies the status because
    /// only the caller knows why the run ended.
    ///
    /// `finished_at` is optional: pass the current time when the run is known to
    /// have ended now, and `None` when the end time is unknown (a row found open
    /// at startup stopped at some unknown point in the past, and stamping it with
    /// the present would report a duration of days). Returns the number of rows
    /// closed.
    pub async fn close_open(
        pool: &SqlitePool,
        cluster_id: &str,
        status: &str,
        finished_at: Option<NaiveDateTime>,
    ) -> Result<u64> {
        match sqlx::query!(
            r#"
                UPDATE task_runs
                SET status = ?, finished_at = ?
                WHERE cluster_id = ? AND status = 'running'
            "#,
            status,
            finished_at,
            cluster_id
        )
        .execute(pool)
        .await
        {
            Ok(result) => Ok(result.rows_affected()),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
    }

    /// How many runs exist, optionally narrowed to one cluster. Used to tell the
    /// caller when a listing has been truncated, so a limit is never silent.
    pub async fn count(pool: &SqlitePool, cluster_id: Option<&str>) -> Result<i64> {
        let result = match cluster_id {
            Some(cid) => {
                sqlx::query_scalar!(
                    r#"SELECT COUNT(*) as "count!: i64" FROM task_runs WHERE cluster_id = ?"#,
                    cid
                )
                .fetch_one(pool)
                .await
            }
            None => {
                sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!: i64" FROM task_runs"#)
                    .fetch_one(pool)
                    .await
            }
        };
        match result {
            Ok(n) => Ok(n),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
    }

    /// Most recent runs first, optionally narrowed to one cluster. A `limit` of
    /// zero or less means no limit (SQLite treats a negative LIMIT as unbounded).
    pub async fn fetch_recent(
        pool: &SqlitePool,
        cluster_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TaskRunRecord>> {
        let limit = if limit <= 0 { -1 } else { limit };
        let result = match cluster_id {
            Some(cid) => {
                sqlx::query_as!(
                    TaskRunRecord,
                    r#"
                    SELECT
                        id as "id!",
                        cluster_id,
                        task_name,
                        run_index,
                        status,
                        started_at as "started_at!",
                        finished_at
                    FROM task_runs
                    WHERE cluster_id = ?
                    ORDER BY started_at DESC, id DESC
                    LIMIT ?
                    "#,
                    cid,
                    limit
                )
                .fetch_all(pool)
                .await
            }
            None => {
                sqlx::query_as!(
                    TaskRunRecord,
                    r#"
                    SELECT
                        id as "id!",
                        cluster_id,
                        task_name,
                        run_index,
                        status,
                        started_at as "started_at!",
                        finished_at
                    FROM task_runs
                    ORDER BY started_at DESC, id DESC
                    LIMIT ?
                    "#,
                    limit
                )
                .fetch_all(pool)
                .await
            }
        };

        match result {
            Ok(rows) => Ok(rows),
            Err(e) => anyhow::bail!("DB Operation Failure: {}", e),
        }
    }
}
