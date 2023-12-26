from django.db import models
from django.utils import timezone


class MPIExperiment(models.Model):
    label = models.TextField(
        null=False,
        blank=False,
    )
    launched_at = models.DateTimeField(
        default=timezone.now,
    )
    completed_at = models.DateTimeField(
        null=True,
    )
    cluster_size = models.IntegerField(
        default=1,
        null=False,
    )
    cluster_has_efa = models.BooleanField(
        default=False,
        null=False,
    )
    cluster_has_fsx = models.BooleanField(
        default=False,
        null=False,
    )
    cluster_is_ephemeral = models.BooleanField(
        default=False,
        null=False,
    )
    cluster_instance_type = models.TextField(
        null=False,
        blank=False,
    )
    ft_technology = models.TextField(
        default="No FT",
        null=False,
        blank=False,
    )
    ckpt_strategy = models.TextField(
        default="No FT",
        null=False,
        blank=False,
    )
    number_of_failures = models.IntegerField(
        default=0,
        null=False,
    )
    job_successfully_completed = models.BooleanField(
        default=False,
        null=False,
    )
    time_spent_spawning_cluster = models.IntegerField(
        default=0,
        null=False,
    )
    time_spent_setting_up_job = models.IntegerField(
        default=0,
        null=False,
    )
    time_spent_checkpointing = models.IntegerField(
        default=0,
        null=False,
    )
    time_spent_restoring_cluster = models.IntegerField(
        default=0,
        null=False,
    )
    time_spent_executing = models.IntegerField(
        default=0,
        null=False,
    )
    total_time_spent = models.IntegerField(
        default=0,
        null=False,
    )
