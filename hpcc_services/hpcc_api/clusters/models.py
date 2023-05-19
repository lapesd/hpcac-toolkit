from django.db import models

from hpcc_api.clusters.managers import ClusterManager


class Cluster(models.Model):
    id = models.AutoField(primary_key=True)
    label = models.TextField(null=False, blank=False)
    is_active = models.BooleanField()

    def __str__(self):
        return f"Cluster {self.id}"

    objects = ClusterManager()
