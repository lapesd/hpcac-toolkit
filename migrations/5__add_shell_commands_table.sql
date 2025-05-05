-- Create the SHELL_COMMANDS table
CREATE TABLE shell_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ordering INTEGER NOT NULL,
    node_id VARCHAR(32) NOT NULL,
    script TEXT NOT NULL,
    status TEXT NOT NULL,
    result TEXT NULL,
    triggered_at DATETIME NULL,
    execution_time INTEGER NULL,
    FOREIGN KEY (node_id) REFERENCES nodes(id)
);
