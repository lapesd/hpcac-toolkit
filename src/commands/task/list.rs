use crate::database::models::TaskRun;

use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePool;
use tabled::{Table, Tabled, settings::Style};

#[derive(Tabled)]
struct TaskRunDisplay {
    #[tabled(rename = "ID")]
    id: i64,
    #[tabled(rename = "Cluster")]
    cluster_id: String,
    #[tabled(rename = "Task")]
    task_name: String,
    #[tabled(rename = "Run")]
    run_index: i64,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Started At")]
    started_at: String,
    #[tabled(rename = "Duration")]
    duration: String,
}

/// How long ago a timestamp was, in the largest unit that still reads naturally:
/// seconds under a minute, then minutes, then hours, then days.
fn relative_time(when: NaiveDateTime) -> String {
    let seconds = (chrono::Utc::now().naive_utc() - when).num_seconds();

    // A row can be stamped slightly ahead of this machine's clock; report it as
    // current rather than as a negative age.
    if seconds < 0 {
        return "just now".to_string();
    }

    let (count, unit) = if seconds < 60 {
        (seconds, "second")
    } else if seconds < 60 * 60 {
        (seconds / 60, "minute")
    } else if seconds < 24 * 60 * 60 {
        (seconds / (60 * 60), "hour")
    } else {
        (seconds / (24 * 60 * 60), "day")
    };

    format!(
        "{} {}{} ago",
        count,
        unit,
        if count == 1 { "" } else { "s" }
    )
}

/// Wall-clock span as minutes and seconds. Clamped at zero, since a run stamped
/// by a machine slightly ahead of this one would otherwise render as negative.
fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    format!("{}m {:02}s", seconds / 60, seconds % 60)
}

pub async fn list(pool: &SqlitePool, cluster_id: Option<&str>, limit: i64) -> Result<()> {
    let total = TaskRun::count(pool, cluster_id).await?;
    let runs = TaskRun::fetch_recent(pool, cluster_id, limit).await?;

    if runs.is_empty() {
        match cluster_id {
            Some(id) => tracing::info!("No task runs found for Cluster '{}'.", id),
            None => tracing::info!("No task runs found."),
        }
        return Ok(());
    }

    let table_rows: Vec<TaskRunDisplay> = runs
        .into_iter()
        .map(|run| {
            let duration = match run.finished_at {
                Some(finished) => format_duration((finished - run.started_at).num_seconds()),
                // A live run has no end time yet, so report how long it has been
                // going. The Status column says 'running', which is what marks
                // this as elapsed rather than final.
                None if run.status == "running" => {
                    format_duration((chrono::Utc::now().naive_utc() - run.started_at).num_seconds())
                }
                // Orphaned, interrupted or cancelled without an end time: we know
                // it stopped, but not when.
                None => "-".to_string(),
            };
            TaskRunDisplay {
                id: run.id,
                cluster_id: run.cluster_id,
                task_name: run.task_name,
                run_index: run.run_index,
                status: run.status,
                started_at: relative_time(run.started_at),
                duration,
            }
        })
        .collect();

    let shown = table_rows.len() as i64;
    let mut table = Table::new(table_rows);
    table.with(Style::rounded());

    // Never truncate silently: a listing that hides 600 rows without saying so
    // is worse than no listing at all.
    let heading = if shown < total {
        format!(
            "\nTask Runs (showing {} of {}; use --limit to see more, --limit 0 for all):",
            shown, total
        )
    } else {
        format!("\nTask Runs ({}):", total)
    };
    tracing::info!("{}\n{}", heading, table);
    Ok(())
}
