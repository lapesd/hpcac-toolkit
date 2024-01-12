from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        CREATE TABLE IF NOT EXISTS "cluster" (
    "cluster_tag" VARCHAR(50) NOT NULL  PRIMARY KEY,
    "provider" VARCHAR(50) NOT NULL,
    "node_instance_type" VARCHAR(50) NOT NULL,
    "instance_username" VARCHAR(50) NOT NULL,
    "node_count" INT NOT NULL,
    "vcpus_per_node" INT NOT NULL,
    "memory_per_node" INT NOT NULL,
    "on_demand_price_per_hour" DOUBLE PRECISION NOT NULL  DEFAULT 0,
    "is_transient" BOOL NOT NULL  DEFAULT False,
    "use_efs" BOOL NOT NULL  DEFAULT True,
    "use_fsx" BOOL NOT NULL  DEFAULT False,
    "use_efa" BOOL NOT NULL  DEFAULT False
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
