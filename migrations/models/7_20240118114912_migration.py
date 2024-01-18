from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ALTER COLUMN "on_demand_price_per_hour" SET DEFAULT '0';
        ALTER TABLE "cluster" ALTER COLUMN "on_demand_price_per_hour" TYPE DECIMAL(12,4) USING "on_demand_price_per_hour"::DECIMAL(12,4);
        ALTER TABLE "cluster" ALTER COLUMN "on_demand_price_per_hour" TYPE DECIMAL(12,4) USING "on_demand_price_per_hour"::DECIMAL(12,4);"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ALTER COLUMN "on_demand_price_per_hour" TYPE DOUBLE PRECISION USING "on_demand_price_per_hour"::DOUBLE PRECISION;
        ALTER TABLE "cluster" ALTER COLUMN "on_demand_price_per_hour" SET DEFAULT 0;"""
