CREATE TABLE task_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id VARCHAR(32) NOT NULL,
    task_name TEXT NOT NULL,
    run_index INTEGER NOT NULL,
    status TEXT NOT NULL,
    started_at DATETIME NOT NULL,
    finished_at DATETIME NULL,
    FOREIGN KEY (cluster_id) REFERENCES clusters(id)
);
