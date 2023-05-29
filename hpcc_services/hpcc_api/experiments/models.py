from django.db import models
from django.utils import timezone

from hpcc_api.clusters.models import ClusterConfiguration


class MPIExperiment(models.Model):
    STATUS_CHOICES = [
        ("starting", "Starting"),
        ("running", "Running"),
        ("checkpointing", "Checkpointing"),
        ("restarting", "Restarting"),
        ("completed", "Completed"),
    ]

    FT_STRATEGY_CHOICES = [
        ("BLCR", "Berkeley-Labs Checkpoint/Restart"),
        ("ULFM", "User-Level Failure Mitigation"),
    ]

    label = models.TextField(
        unique=True,
        null=False,
        blank=False,
    )
    github_repository = models.URLField(
        null=False,
        blank=False,
    )
    ft_strategy = models.CharField(
        max_length=10,
        choices=FT_STRATEGY_CHOICES,
        null=True,
    )
    status = models.CharField(
        max_length=20,
        choices=STATUS_CHOICES,
        default="starting",
    )
    started_at = models.DateTimeField(default=timezone.now)
    completed_at = models.DateTimeField(null=True)
    cluster_configuration = models.ForeignKey(
        ClusterConfiguration, null=True, on_delete=models.SET_NULL
    )

    def __str__(self):
        return f"MPIExperiment {self.id}"
