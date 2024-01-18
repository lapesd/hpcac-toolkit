from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" RENAME COLUMN "is_transient" TO "use_spot";"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "cluster" RENAME COLUMN "use_spot" TO "is_transient";"""
