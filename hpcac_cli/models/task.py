from decimal import Decimal
from enum import Enum

from tortoise.models import Model
from tortoise import fields

from hpcac_cli.utils.logger import Logger


log = Logger()


class TaskStatus(Enum):
    Success = 1
    NodeEvicted = 2
    RemoteException = 3
    NotCompleted = 4


class Task(Model):
    task_tag = fields.CharField(pk=True, unique=True, max_length=50)
    cluster = fields.ForeignKeyField(
        "models.Cluster", related_name="tasks", to_field="cluster_tag"
    )
    created_at = fields.DatetimeField(auto_now_add=True)
    started_at = fields.DatetimeField(null=True)
    completed_at = fields.DatetimeField(null=True)
    failures_during_execution = fields.IntField(default=0)
    retries_before_aborting = fields.IntField(default=0)
    fault_tolerance_technology_label = fields.CharField(default="noft", max_length=50)
    checkpoint_strategy_label = fields.CharField(default="noft", max_length=50)
    task_completed_successfully = fields.BooleanField(default=False)
    time_spent_spawning_cluster = fields.IntField(default=0)
    time_spent_setting_up_task = fields.IntField(default=0)
    time_spent_checkpointing = fields.IntField(default=0)
    time_spent_restoring_cluster = fields.IntField(default=0)
    time_spent_executing_task = fields.IntField(default=0)
    approximate_costs = fields.DecimalField(
        max_digits=12, decimal_places=4, default=Decimal(0.0)
    )
    setup_command = fields.TextField()
    run_command = fields.TextField()
    checkpoint_command = fields.TextField(null=True)
    restart_command = fields.TextField(null=True)
    remote_outputs_dir = fields.TextField()


async def is_task_tag_alredy_used(task_tag: str) -> bool:
    existing_task = await Task.filter(task_tag=task_tag).first()
    return True if existing_task else False


async def insert_task_record(task_data: dict, overwrite: bool = False) -> Task:
    # Delete existing Task if overwrite == True:
    if overwrite and await is_task_tag_alredy_used(task_tag=task_data["task_tag"]):
        await Task.filter(task_tag=task_data["task_tag"]).delete()

    # Create new Task record:
    task = await Task.create(**task_data)
    if overwrite:
        log.info(f"Overwritten Task `{task.task_tag}` details into Postgres!")
    else:
        log.info(f"Inserted Task `{task.task_tag}` details into Postgres!")

    return task
