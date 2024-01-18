from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ADD "region" VARCHAR(50) NOT NULL;
        ALTER TABLE "cluster" ADD "node_ips" JSONB NOT NULL;"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" DROP COLUMN "region";
        ALTER TABLE "cluster" DROP COLUMN "node_ips";"""
