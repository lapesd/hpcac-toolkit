import pytest

from hpcc_api.clusters.models import ClusterConfiguration


@pytest.mark.django_db(databases=["default"])
def test_cluster_model():
    assert True
