from rest_framework import viewsets

from hpcc_api.clusters.models import ClusterConfiguration


class ClusterViewSet(viewsets.ModelViewSet):
    queryset = ClusterConfiguration.objects.all()
