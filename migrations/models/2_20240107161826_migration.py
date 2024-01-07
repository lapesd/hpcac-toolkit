from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ADD "use_efa" BOOL NOT NULL  DEFAULT False;
        ALTER TABLE "cluster" ADD "instance_username" VARCHAR(50) NOT NULL;
        ALTER TABLE "cluster" RENAME COLUMN "nodes" TO "node_count";"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" RENAME COLUMN "node_count" TO "nodes";
        ALTER TABLE "cluster" DROP COLUMN "use_efa";
        ALTER TABLE "cluster" DROP COLUMN "instance_username";"""
