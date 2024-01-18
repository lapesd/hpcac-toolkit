from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" ALTER COLUMN "started_at" DROP NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "completed_at" DROP NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "approximate_costs" SET DEFAULT '0';"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" ALTER COLUMN "started_at" SET NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "completed_at" SET NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "approximate_costs" DROP DEFAULT;"""
