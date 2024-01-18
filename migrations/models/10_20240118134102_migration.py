from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" ALTER COLUMN "restart_command" DROP NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "ckpt_command" DROP NOT NULL;"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" ALTER COLUMN "restart_command" SET NOT NULL;
        ALTER TABLE "task" ALTER COLUMN "ckpt_command" SET NOT NULL;"""
