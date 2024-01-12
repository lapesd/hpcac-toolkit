from tortoise.models import Model
from tortoise import fields

from hpcac_cli.utils.logger import info


BOOLEANS = ["use_spot", "use_efs", "use_fsx", "use_efa"]


class Cluster(Model):
    cluster_tag = fields.CharField(pk=True, unique=True, max_length=50)
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

    def __str__(self):
        return (
            f"Cluster {self.cluster_tag}: {self.node_count}x {self.node_instance_type}"
        )


async def upsert_cluster(cluster_data: dict):
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

    cluster_tag = cluster_data["cluster_tag"]

    # Check if a Cluster with the given cluster_tag already exists
    existing_cluster = await Cluster.filter(cluster_tag=cluster_tag).first()
    if existing_cluster:
        # If exists, delete the existing record
        await existing_cluster.delete()
        info(f"Deleted existing `{cluster_tag}` Cluster details from Postgres.")

    # Create a new record
    for key in BOOLEANS:
        cluster_data[key] = True if cluster_data[key] == "true" else False

    await Cluster.create(**cluster_data)
    info(f"Inserted new `{cluster_tag}` Cluster details into Postgres!")
