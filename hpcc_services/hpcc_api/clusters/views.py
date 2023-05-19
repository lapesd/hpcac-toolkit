from rest_framework import viewsets

from hpcc_api.clusters.models import Cluster


class ClusterViewSet(viewsets.ModelViewSet):
    queryset = Cluster.objects.active()
