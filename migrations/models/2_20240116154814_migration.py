from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        CREATE TABLE IF NOT EXISTS "task" (
    "task_tag" VARCHAR(50) NOT NULL  PRIMARY KEY,
    "created_at" TIMESTAMPTZ NOT NULL  DEFAULT CURRENT_TIMESTAMP,
    "started_at" TIMESTAMPTZ NOT NULL,
    "completed_at" TIMESTAMPTZ NOT NULL,
    "failures_during_execution" INT NOT NULL  DEFAULT 0,
    "retries_before_aborting" INT NOT NULL  DEFAULT 0,
    "ft_technology" VARCHAR(50) NOT NULL  DEFAULT 'noft',
    "ckpt_strategy" VARCHAR(50) NOT NULL  DEFAULT 'noft',
    "task_completed_successfully" BOOL NOT NULL  DEFAULT False,
    "time_spent_spawning_cluster" INT NOT NULL  DEFAULT 0,
    "time_spent_setting_up_task" INT NOT NULL  DEFAULT 0,
    "time_spent_checkpointing" INT NOT NULL  DEFAULT 0,
    "time_spent_restoring_cluster" INT NOT NULL  DEFAULT 0,
    "time_spent_executing_task" INT NOT NULL  DEFAULT 0,
    "approximate_costs" DECIMAL(12,4) NOT NULL,
    "cluster_id" VARCHAR(50) NOT NULL REFERENCES "cluster" ("cluster_tag") ON DELETE CASCADE
);"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        DROP TABLE IF EXISTS "task";"""
