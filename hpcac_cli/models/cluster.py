from tortoise.queryset import QuerySet
from tortoise.models import Model
from tortoise import fields

from hpcac_cli.utils.logger import info


BOOLEANS = ["use_spot", "use_efs", "use_fsx", "use_efa"]


class Cluster(Model):
    cluster_tag = fields.CharField(pk=True, unique=True, max_length=50)
    created_at = fields.DatetimeField(auto_now_add=True)
    is_online = fields.BooleanField(default=False)
    provider = fields.CharField(max_length=50)
    node_instance_type = fields.CharField(max_length=50)
    instance_username = fields.CharField(max_length=50)
    node_count = fields.IntField()
    vcpus_per_node = fields.IntField()
    memory_per_node = fields.IntField()
    on_demand_price_per_hour = fields.FloatField(default=0.0)
    is_transient = fields.BooleanField(default=False)
    use_efs = fields.BooleanField(default=True)
    use_fsx = fields.BooleanField(default=False)
    use_efa = fields.BooleanField(default=False)
    time_spent_spawning_cluster = fields.IntField(default=0)

    def __str__(self):
        return (
            f"Cluster {self.cluster_tag}: {self.node_count}x {self.node_instance_type}"
        )


async def is_cluster_tag_alredy_used(cluster_tag: str) -> bool:
    existing_cluster = await Cluster.filter(cluster_tag=cluster_tag).first()
    return True if existing_cluster else False


async def insert_cluster_record(cluster_data: dict) -> Cluster:
    # Ensure all required keys are present in the dictionary
    required_keys = {
        "cluster_tag",
        "node_instance_type",
        "node_count",
        "instance_username",
        "vcpus_per_node",
        "memory_per_node",
        "provider",
    }
    if not required_keys.issubset(cluster_data.keys()):
        raise ValueError(
            "Missing required keys in cluster_data. "
            f"Required keys are: {required_keys}"
        )
    for key in BOOLEANS:
        cluster_data[key] = True if cluster_data[key] == "true" else False

    # Create new Cluster record:
    cluster = await Cluster.create(**cluster_data)
    info(f"Inserted new `{cluster_data['cluster_tag']}` Cluster details into Postgres!")
    return cluster


async def fetch_latest_online_cluster() -> QuerySet[Cluster]:
    latest_cluster = (
        await Cluster.filter(is_online=True).order_by("-created_at").first()
    )
    if latest_cluster:
        return latest_cluster
    else:
        raise Exception("No online clusters available.")
