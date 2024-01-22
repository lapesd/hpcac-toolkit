import concurrent.futures
from decimal import Decimal
import os
import time

from tortoise.queryset import QuerySet
from tortoise.models import Model
from tortoise import fields

from hpcac_cli.utils.logger import info
from hpcac_cli.utils.providers.aws import (
    get_cluster_efs_dns_name,
    get_cluster_nodes_ip_addresses,
)
from hpcac_cli.utils.ssh import (
    ping,
    remote_command,
    scp_transfer_directory,
    scp_download_directory,
)
from hpcac_cli.utils.terraform import terraform_refresh, terraform_apply

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
    on_demand_price_per_hour = fields.DecimalField(
        max_digits=12, decimal_places=4, default=Decimal(0.0)
    )

    def __str__(self):
        return (
            f"Cluster {self.cluster_tag}: {self.node_count}x {self.node_instance_type}"
        )

    def run_command(
        self,
        command: str,
        ip_list_to_run: list[str],
        raise_exception: bool = False,
    ):
        def task(ip):
            return remote_command(
                ip=ip, username=self.instance_username, command=command
            )

        with concurrent.futures.ThreadPoolExecutor() as executor:
            future_to_ip = {executor.submit(task, ip): ip for ip in ip_list_to_run}
            for future in concurrent.futures.as_completed(future_to_ip):
                _ip = future_to_ip[future]
                result = future.result()
                if raise_exception and not result:
                    raise Exception(f"Aborting...")

    def is_healthy(self) -> bool:
        def ping_node(ip):
            return ping(ip=ip, username=self.instance_username)

        with concurrent.futures.ThreadPoolExecutor() as executor:
            future_to_ip = {executor.submit(ping_node, ip): ip for ip in self.node_ips}
            for future in concurrent.futures.as_completed(future_to_ip):
                ip = future_to_ip[future]
                is_alive = future.result()
                if not is_alive:
                    info(
                        f"Cluster `{self.cluster_tag}` is NOT healthy due to node: {ip}"
                    )
                    return False
        return True

    def generate_hostfile(self, mpi_distribution: str):
        info(
            f"Generating {mpi_distribution} hostfile for Cluster `{self.cluster_tag}`..."
        )
        if mpi_distribution.lower() != "openmpi":
            raise NotImplementedError(
                f"Hostfile generation for {mpi_distribution} not implemented."
            )

        # Generate Hostfile for OpenMPI:
        base_host = "10.0.0.1"
        HOSTFILE_PATH = "./my_files/hostfile"
        if os.path.exists(HOSTFILE_PATH):
            os.remove(HOSTFILE_PATH)
        with open(HOSTFILE_PATH, "w") as file:
            for i in range(self.node_count):
                file.write(f"{base_host}{i} slots={self.vcpus_per_node}\n")

    def upload_my_files(self):
        # First make sure the remote `my_files` directory exists:
        self.run_command(
            "mkdir -p /var/nfs_dir/my_files", ip_list_to_run=[self.node_ips[0]]
        )

        # Then upload the local my_files contents:
        LOCAL_MY_FILES_PATH = "./my_files"
        REMOTE_MY_FILES_PATH = "/var/nfs_dir/"
        scp_transfer_directory(
            local_path=LOCAL_MY_FILES_PATH,
            remote_path=REMOTE_MY_FILES_PATH,
            ip=self.node_ips[0],
            username=self.instance_username,
        )

    def setup_efs(self, ip_list_to_run: list[str]):
        if self.provider != "aws":
            raise NotImplementedError(
                "Setup EFS is currently only implemented for AWS."
            )

        efs_dns_name = get_cluster_efs_dns_name(
            cluster_tag=self.cluster_tag, region=self.region
        )
        commands = [
            "sudo yum install -y nfs-utils",
            "sudo mkdir -p /var/nfs_dir",
        ]
        for command in commands:
            self.run_command(
                command=command,
                ip_list_to_run=ip_list_to_run,
                raise_exception=True,
            )

        # Mount EFS to /var/nfs_dir
        mount_command = f"sudo mount -t nfs {efs_dns_name}:/ /var/nfs_dir"
        mounted = False
        while not mounted:
            for ip in self.node_ips:
                mounted = remote_command(
                    ip=ip, username=self.instance_username, command=mount_command
                )
                time.sleep(5)

        commands = [
            "sudo chmod ugo+rwx /var/nfs_dir",
            f"sudo bash -c 'echo \"{efs_dns_name}:/ /var/nfs_dir nfs defaults,_netdev 0 0\" >> /etc/fstab'",
        ]
        for command in commands:
            self.run_command(
                command=command,
                ip_list_to_run=ip_list_to_run,
                raise_exception=True,
            )

    def clean_my_files(self):
        info(f"Cleaning remote contents at `/var/nfs_dir/my_files`...")
        self.run_command(
            "if [ -d /var/nfs_dir/my_files ]; then rm -r /var/nfs_dir/my_files; fi",
            ip_list_to_run=[self.node_ips[0]],
        )

    def download_directory(self, remote_path: str, local_path: str):
        scp_download_directory(
            local_path=local_path,
            remote_path=remote_path,
            ip=self.node_ips[0],
            username=self.instance_username,
        )

    async def repair(self):
        info(f"Repairing Cluster `{self.cluster_tag}`...")
        old_ips = self.node_ips
        while not self.is_healthy():
            terraform_refresh()
            terraform_apply()
            new_ips = get_cluster_nodes_ip_addresses(
                cluster_tag=self.cluster_tag, region=self.region
            )
            self.node_ips = new_ips
            await self.save()

        new_nodes_ips = [ip for ip in new_ips if ip not in old_ips]
        if self.use_efs:
            # Reconnect destroyed nodes to EFS:
            self.setup_efs(ip_list_to_run=new_nodes_ips)

        info(f"Repaired cluster `{self.cluster_tag}!")


async def is_cluster_tag_alredy_used(cluster_tag: str) -> bool:
    existing_cluster = await Cluster.filter(cluster_tag=cluster_tag).first()
    return True if existing_cluster else False


async def insert_cluster_record(cluster_data: dict) -> Cluster:
    # Filter out keys not in the Cluster model
    cluster_model_fields = {f for f in Cluster._meta.fields_map}
    filtered_cluster_data = {
        k: v for k, v in cluster_data.items() if k in cluster_model_fields
    }

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
        filtered_cluster_data[key] = (
            True if filtered_cluster_data[key] == "true" else False
        )

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
