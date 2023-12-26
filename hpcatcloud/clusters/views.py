from rest_framework import viewsets

from hpcatcloud.clusters.models import ClusterConfiguration


class ClusterViewSet(viewsets.ModelViewSet):
    queryset = ClusterConfiguration.objects.all()
