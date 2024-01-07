from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        CREATE TABLE IF NOT EXISTS "cluster" (
    "tag" VARCHAR(50) NOT NULL  PRIMARY KEY,
    "provider" VARCHAR(50) NOT NULL,
    "instance_type" VARCHAR(50) NOT NULL,
    "nodes" INT NOT NULL,
    "vcpus_per_node" INT NOT NULL,
    "memory_per_node" INT NOT NULL,
    "is_transient" BOOL NOT NULL  DEFAULT False,
    "use_efs" BOOL NOT NULL  DEFAULT True,
    "use_fsx" BOOL NOT NULL  DEFAULT False
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
