import pytest

from hpcc_api.clusters.models import Cluster


@pytest.mark.django_db(databases=["default"])
def test_cluster_model():

    assert True
