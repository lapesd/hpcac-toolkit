from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        CREATE TABLE IF NOT EXISTS "cluster" (
    "cluster_tag" VARCHAR(50) NOT NULL  PRIMARY KEY,
    "created_at" TIMESTAMPTZ NOT NULL  DEFAULT CURRENT_TIMESTAMP,
    "is_online" BOOL NOT NULL  DEFAULT False,
    "provider" VARCHAR(50) NOT NULL,
    "region" VARCHAR(50) NOT NULL,
    "node_instance_type" VARCHAR(50) NOT NULL,
    "instance_username" VARCHAR(50) NOT NULL,
    "node_count" INT NOT NULL,
    "vcpus_per_node" INT NOT NULL,
    "memory_per_node" INT NOT NULL,
    "use_spot" BOOL NOT NULL  DEFAULT False,
    "use_efs" BOOL NOT NULL  DEFAULT True,
    "use_fsx" BOOL NOT NULL  DEFAULT False,
    "use_efa" BOOL NOT NULL  DEFAULT False,
    "node_ips" JSONB NOT NULL,
    "init_commands" JSONB NOT NULL,
    "time_spent_spawning_cluster" INT NOT NULL  DEFAULT 0,
    "on_demand_price_per_hour" DECIMAL(12,4) NOT NULL  DEFAULT 0
);
CREATE TABLE IF NOT EXISTS "task" (
    "task_tag" VARCHAR(50) NOT NULL  PRIMARY KEY,
    "created_at" TIMESTAMPTZ NOT NULL  DEFAULT CURRENT_TIMESTAMP,
    "started_at" TIMESTAMPTZ,
    "completed_at" TIMESTAMPTZ,
    "failures_during_execution" INT NOT NULL  DEFAULT 0,
    "retries_before_aborting" INT NOT NULL  DEFAULT 0,
    "fault_tolerance_technology_label" VARCHAR(50) NOT NULL  DEFAULT 'noft',
    "checkpoint_strategy_label" VARCHAR(50) NOT NULL  DEFAULT 'noft',
    "task_completed_successfully" BOOL NOT NULL  DEFAULT False,
    "time_spent_spawning_cluster" INT NOT NULL  DEFAULT 0,
    "time_spent_setting_up_task" INT NOT NULL  DEFAULT 0,
    "time_spent_checkpointing" INT NOT NULL  DEFAULT 0,
    "time_spent_restoring_cluster" INT NOT NULL  DEFAULT 0,
    "time_spent_executing_task" INT NOT NULL  DEFAULT 0,
    "approximate_costs" DECIMAL(12,4) NOT NULL  DEFAULT 0,
    "setup_command" TEXT NOT NULL,
    "run_command" TEXT NOT NULL,
    "checkpoint_command" TEXT,
    "restart_command" TEXT,
    "remote_outputs_dir" TEXT NOT NULL,
    "cluster_id" VARCHAR(50) NOT NULL REFERENCES "cluster" ("cluster_tag") ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS "aerich" (
    "id" SERIAL NOT NULL PRIMARY KEY,
    "version" VARCHAR(255) NOT NULL,
    "app" VARCHAR(100) NOT NULL,
    "content" JSONB NOT NULL
);"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        """
