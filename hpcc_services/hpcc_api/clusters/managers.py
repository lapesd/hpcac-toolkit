from django.db import models


class ClusterManager(models.Manager):
    def active(self):
        return self.filter(is_active=True)
