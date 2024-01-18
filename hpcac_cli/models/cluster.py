from decimal import Decimal

from tortoise.queryset import QuerySet
from tortoise.models import Model
from tortoise import fields

from hpcac_cli.utils.logger import info
from hpcac_cli.utils.ssh import ping


DECIMALS = ["on_demand_price_per_hour"]
BOOLEANS = ["use_spot", "use_efs", "use_fsx", "use_efa"]


class Cluster(Model):
    cluster_tag = fields.CharField(pk=True, unique=True, max_length=50)
    created_at = fields.DatetimeField(auto_now_add=True)
    is_online = fields.BooleanField(default=False)
    provider = fields.CharField(max_length=50)
    region = fields.CharField(max_length=50)
    node_instance_type = fields.CharField(max_length=50)
    instance_username = fields.CharField(max_length=50)
    node_count = fields.IntField()
    vcpus_per_node = fields.IntField()
    memory_per_node = fields.IntField()
    use_spot = fields.BooleanField(default=False)
    use_efs = fields.BooleanField(default=True)
    use_fsx = fields.BooleanField(default=False)
    use_efa = fields.BooleanField(default=False)
    node_ips = fields.JSONField(default=list)
    time_spent_spawning_cluster = fields.IntField(default=0)
    on_demand_price_per_hour = fields.DecimalField(max_digits=12, decimal_places=4, default=Decimal(0.0))

    def __str__(self):
        return (
            f"Cluster {self.cluster_tag}: {self.node_count}x {self.node_instance_type}"
        )
    
    def is_healthy(self) -> bool:
        for ip in self.node_ips:
            is_alive = ping(ip=ip, username=self.instance_username)
            if not is_alive:
                info(f"Cluster `{self.cluster_tag}` is NOT healthy!")
                return False
        info(f"Cluster `{self.cluster_tag}` is healthy!")
        return True


async def is_cluster_tag_alredy_used(cluster_tag: str) -> bool:
    existing_cluster = await Cluster.filter(cluster_tag=cluster_tag).first()
    return True if existing_cluster else False


async def insert_cluster_record(cluster_data: dict) -> Cluster:
    # Filter out keys not in the Cluster model
    cluster_model_fields = {f for f in Cluster._meta.fields_map}
    filtered_cluster_data = {k: v for k, v in cluster_data.items() if k in cluster_model_fields}

    # Ensure all required keys are present in the dictionary
    required_keys = {
        "cluster_tag",
        "node_instance_type",
        "node_count",
        "instance_username",
        "vcpus_per_node",
        "memory_per_node",
        "provider",
        "region",
    }
    if not required_keys.issubset(filtered_cluster_data.keys()):
        raise ValueError(
            "Missing required keys in cluster_data. "
            f"Required keys are: {required_keys}"
        )

    # Convert Decimals:
    for key in DECIMALS:
        filtered_cluster_data[key] = Decimal(filtered_cluster_data[key])

    # Convert booleans:
    for key in BOOLEANS:
        filtered_cluster_data[key] = True if filtered_cluster_data[key] == "true" else False

    # Create new Cluster record:
    cluster = await Cluster.create(**filtered_cluster_data)
    info(f"Inserted new `{cluster.cluster_tag}` Cluster details into Postgres!")
    return cluster


async def fetch_latest_online_cluster() -> QuerySet[Cluster]:
    latest_cluster = (
        await Cluster.filter(is_online=True).order_by("-created_at").first()
    )
    if latest_cluster:
        return latest_cluster
    else:
        raise Exception("No online clusters available.")
