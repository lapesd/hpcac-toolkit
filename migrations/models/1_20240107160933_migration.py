from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" RENAME COLUMN "instance_type" TO "node_instance_type";
        ALTER TABLE "cluster" ADD "on_demand_price_per_hour" DOUBLE PRECISION NOT NULL  DEFAULT 0;"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" RENAME COLUMN "node_instance_type" TO "instance_type";
        ALTER TABLE "cluster" DROP COLUMN "on_demand_price_per_hour";"""
