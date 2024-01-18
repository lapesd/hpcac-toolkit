from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ALTER COLUMN "node_ips" DROP DEFAULT;"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" ALTER COLUMN "node_ips" SET;"""
