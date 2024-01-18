from tortoise import BaseDBAsyncClient


async def upgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" ADD "setup_command" TEXT NOT NULL;
        ALTER TABLE "task" ADD "remote_outputs_dir" TEXT NOT NULL;
        ALTER TABLE "task" ADD "ckpt_command" TEXT NOT NULL;
        ALTER TABLE "task" ADD "run_command" TEXT NOT NULL;
        ALTER TABLE "task" ADD "restart_command" TEXT NOT NULL;"""


async def downgrade(db: BaseDBAsyncClient) -> str:
    return """
        ALTER TABLE "task" DROP COLUMN "setup_command";
        ALTER TABLE "task" DROP COLUMN "remote_outputs_dir";
        ALTER TABLE "task" DROP COLUMN "ckpt_command";
        ALTER TABLE "task" DROP COLUMN "run_command";
        ALTER TABLE "task" DROP COLUMN "restart_command";"""
