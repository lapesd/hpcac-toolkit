from decimal import Decimal
import os
import socket
import time

import paramiko
from paramiko.ssh_exception import NoValidConnectionsError, SSHException
from tortoise.queryset import QuerySet
from tortoise.models import Model
from tortoise import fields

from hpcac_cli.models.task import TaskStatus
from hpcac_cli.utils.logger import Logger
from hpcac_cli.utils.providers.aws import (
    get_cluster_efs_dns_name,
    get_running_nodes_ips,
)
from hpcac_cli.utils.ssh import (
    scp_transfer_directory,
    scp_download_directory,
)
from hpcac_cli.utils.terraform import terraform_refresh


log = Logger()
DECIMALS = ["on_demand_price_per_hour"]
BOOLEANS = ["use_spot", "use_efs", "use_fsx", "use_efa"]


class EFSError(Exception):
    """Base class for exceptions in EFS module."""

    pass


class ClusterInitError(Exception):
    """Base class for cluster_init exceptions."""

    pass


class Cluster(Model):
    cluster_tag = fields.CharField(pk=True, unique=True, max_length=128)
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
    init_commands = fields.JSONField(default=list)
    time_spent_spawning_cluster = fields.IntField(default=0)
    on_demand_price_per_hour = fields.DecimalField(
        max_digits=12, decimal_places=4, default=Decimal(0.0)
    )

    def __str__(self):
        return (
            f"Cluster {self.cluster_tag}: {self.node_count}x {self.node_instance_type}"
        )

    def generate_hostfile(self, mpi_distribution: str):
        log.debug(text=f"Generating {mpi_distribution} hostfile...")
        if mpi_distribution.lower() != "openmpi":
            raise NotImplementedError(
                f"Hostfile generation for {mpi_distribution} not implemented."
            )
        base_host = "10.0.0.1"
        HOSTFILE_PATH = "./my_files/hostfile"
        if os.path.exists(HOSTFILE_PATH):
            os.remove(HOSTFILE_PATH)
        with open(HOSTFILE_PATH, "w") as file:
            for i in range(self.node_count):
                file.write(f"{base_host}{i} slots={self.vcpus_per_node}\n")
        log.debug(text=f"Generation of {mpi_distribution} hostfile complete!")

    def clean_remote_my_files_directory(self):
        command = (
            "if [ -d /var/nfs_dir/my_files ]; then rm -r /var/nfs_dir/my_files; fi"
        )
        raise_text = ""
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        ip = self.node_ips[0]
        try:
            ssh.connect(ip, username=self.instance_username, timeout=3)
        except Exception as err:
            raise_text = f"Raised `{type(err).__name__}` while connecting to node {ip}."
            raise ClusterInitError(raise_text)

        try:
            log.debug(
                text=f"Executing command: ```\n{command}\n```",
                detail=f"clean_remote_my_files_directory@{ip}",
            )
            _stdin, stdout, stderr = ssh.exec_command(command=command)
            exit_status = stdout.channel.recv_exit_status()
            stdout_text = stdout.read().decode().strip()
            stderr_text = stderr.read().decode().strip()
        except (NoValidConnectionsError, SSHException, socket.timeout) as err:
            log.warning(
                f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                detail=f"clean_remote_my_files_directory@{ip}",
            )
            raise_text = (
                f"Raised `{type(err).__name__}` while cleaning remote `my_files`."
            )
        except Exception as err:
            log.error(
                f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                detail=f"clean_remote_my_files_directory@{ip}",
            )
            raise_text = (
                f"Raised `{type(err).__name__}` while cleaning remote `my_files`."
            )
        else:
            if exit_status == 0:
                log.info(
                    f"Command `{command}` executed successfully!",
                    detail=f"clean_remote_my_files_directory@{ip}",
                )
            else:
                log.error(
                    f"Failed running command `{command}`: ```\n{stderr_text}\n```",
                    detail=f"clean_remote_my_files_directory@{ip}",
                )
                raise_text = f"Bad exit_code from command `{command}`"

        ssh.close()
        if stdout_text != "":
            log.debug(
                text=f"STDOUT: ```\n{stdout_text}\n```",
                detail=f"clean_remote_my_files_directory@{ip}",
            )
        if raise_text != "":
            raise ClusterInitError(raise_text)

    def upload_my_files(self):
        LOCAL_MY_FILES_PATH = "./my_files"
        REMOTE_MY_FILES_PATH = "/var/nfs_dir/"
        scp_transfer_directory(
            local_path=LOCAL_MY_FILES_PATH,
            remote_path=REMOTE_MY_FILES_PATH,
            ip=self.node_ips[0],
            username=self.instance_username,
        )

    def download_directory(self, remote_path: str, local_path: str):
        scp_download_directory(
            local_path=local_path,
            remote_path=remote_path,
            ip=self.node_ips[0],
            username=self.instance_username,
        )

    def is_healthy(self) -> bool:
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

        unhealthy_nodes = []
        healthy_nodes = []
        for ip in self.node_ips:
            try:
                ssh.connect(ip, username=self.instance_username, timeout=3)
                _stdin, _stdout, _stderr = ssh.exec_command('echo "I\'m alive!"')
            except (NoValidConnectionsError, SSHException, socket.timeout) as err:
                log.debug(f"Node `{ip}` unreachable: ```\n{err}\n```")
                unhealthy_nodes.append(ip)
            else:
                healthy_nodes.append(ip)
            ssh.close()

        log.debug(f"Healthy nodes: {healthy_nodes}")
        if len(unhealthy_nodes) > 0:
            log.warning(f"Unhealthy nodes: {unhealthy_nodes}")
            return False
        return True

    def run_task(self, command: str) -> TaskStatus:
        ip = self.node_ips[0]
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        task_status = TaskStatus.NotCompleted
        try:
            ssh.connect(ip, username=self.instance_username, timeout=3)
            log.debug(
                text=f"Executing Task command: ```\n{command}\n```",
                detail=f"run_task@{ip}",
            )
            _stdin, stdout, stderr = ssh.exec_command(command=command)
            exit_status = stdout.channel.recv_exit_status()
            stdout_text = stdout.read().decode().strip()
            stderr_text = stderr.read().decode().strip()
            if stdout_text != "":
                log.debug(
                    text=f"STDOUT: ```\n{stdout_text}\n```", detail=f"run_task@{ip}"
                )
        except (NoValidConnectionsError, SSHException, socket.timeout) as err:
            log.warning(f"```\n{err}\n```", detail="TaskStatus=NodeEvicted")
            task_status = TaskStatus.NodeEvicted
        except Exception as err:
            log.error(f"```\n{err}\n```", detail="TaskStatus=RemoteException")
            task_status = TaskStatus.RemoteException
        else:
            if exit_status == 0:
                if "PRTE has lost communication" in stderr_text:
                    log.warning(
                        f"```\n{stderr_text}\n```", detail="TaskStatus=NodeEvicted"
                    )
                    task_status = TaskStatus.NodeEvicted
                else:
                    log.info(
                        f"Completed task command `{command}` successfully!!",
                        detail="TaskStatus=Success",
                    )
                    task_status = TaskStatus.Success
            else:
                # Check if a node was evicted or not
                if not self.is_healthy():
                    log.warning(
                        f"```\n{stderr_text}\n```", detail="TaskStatus=NodeEvicted"
                    )
                    task_status = TaskStatus.NodeEvicted
                else:
                    log.error(
                        f"```\n{stderr_text}\n```", detail="TaskStatus=RemoteException"
                    )
                    task_status = TaskStatus.RemoteException

        ssh.close()
        return task_status

    def setup_efs(self, ip_list_to_run: list[str], wait_time: int = 150):
        if self.provider != "aws":
            raise NotImplementedError(
                "Setup EFS is currently only implemented for AWS."
            )
        # Need to just add a brief sleep condition here to make sure EFS dns is propagated through the VPN
        # https://docs.aws.amazon.com/efs/latest/ug/troubleshooting-efs-mounting.html#mount-fails-propegation
        log.debug(
            text=f"Waiting {wait_time} seconds for AWS Elastic File System DNS to be reachable...",
            detail="setup_efs",
        )
        time.sleep(wait_time)
        log.debug(text=f"Wait of {wait_time} seconds completed.", detail="setup_efs")

        try:
            efs_dns_name = get_cluster_efs_dns_name(
                cluster_tag=self.cluster_tag, region=self.region
            )
            if efs_dns_name is None:
                raise EFSError("Couldn't reach EFS by DNS name.")
        except Exception as err:
            raise EFSError(f"{type(err).__name__} while setting up EFS.")

        efs_setup_commands = [
            "sudo yum install -y nfs-utils",
            "sudo mkdir -p /var/nfs_dir",
            f"sudo mount -t nfs {efs_dns_name}:/ /var/nfs_dir",
            "sudo chmod ugo+rwx /var/nfs_dir",
            f"sudo bash -c 'echo \"{efs_dns_name}:/ /var/nfs_dir nfs defaults,_netdev 0 0\" >> /etc/fstab'",
        ]
        raise_text = ""
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        for ip in ip_list_to_run:
            try:
                ssh.connect(ip, username=self.instance_username, timeout=3)
            except Exception as err:
                raise_text = (
                    f"Raised `{type(err).__name__}` while connecting to node {ip}."
                )
                raise EFSError(raise_text)

            try:
                for command in efs_setup_commands:
                    log.debug(
                        text=f"Executing EFS setup command: ```\n{command}\n```",
                        detail=f"setup_task@{ip}",
                    )
                    _stdin, stdout, stderr = ssh.exec_command(command=command)
                    exit_status = stdout.channel.recv_exit_status()
                    stdout_text = stdout.read().decode().strip()
                    stderr_text = stderr.read().decode().strip()
            except (NoValidConnectionsError, SSHException, socket.timeout) as err:
                log.warning(
                    f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                    detail=f"setup_task@{ip}",
                )
                raise_text = f"Raised `{type(err).__name__}` while setting up EFS."
            except Exception as err:
                log.error(
                    f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                    detail=f"setup_task@{ip}",
                )
                raise_text = f"Raised `{type(err).__name__}` while setting up EFS."
            else:
                if exit_status == 0:
                    log.info(
                        f"EFS setup commands executed successfully!",
                        detail=f"setup_task@{ip}",
                    )
                else:
                    log.error(
                        f"Failed running command `{command}`: ```\n{stderr_text}\n```",
                        detail=f"setup_task@{ip}",
                    )
                    raise_text = f"Bad exit_code from command `{command}`"

            ssh.close()
            if stdout_text != "":
                log.debug(
                    text=f"STDOUT: ```\n{stdout_text}\n```", detail=f"setup_task@{ip}"
                )
            if raise_text != "":
                raise EFSError(raise_text)

    def run_init_commands(self, ip_list_to_run: list[str]):
        raise_text = ""
        ssh = paramiko.SSHClient()
        ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
        for ip in ip_list_to_run:
            try:
                ssh.connect(ip, username=self.instance_username, timeout=3)
            except Exception as err:
                raise_text = (
                    f"Raised `{type(err).__name__}` while connecting to node {ip}."
                )
                raise ClusterInitError(raise_text)

            try:
                for command in self.init_commands:
                    log.debug(
                        text=f"Executing cluster init command: ```\n{command}\n```",
                        detail=f"init_commands@{ip}",
                    )
                    _stdin, stdout, stderr = ssh.exec_command(command=command)
                    exit_status = stdout.channel.recv_exit_status()
                    stdout_text = stdout.read().decode().strip()
                    stderr_text = stderr.read().decode().strip()
            except (NoValidConnectionsError, SSHException, socket.timeout) as err:
                log.warning(
                    f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                    detail=f"init_commands@{ip}",
                )
                raise_text = (
                    f"Raised `{type(err).__name__}` while running init_commands."
                )
            except Exception as err:
                log.error(
                    f"{type(err).__name__} running `{command}`: ```\n{err}\n```",
                    detail=f"init_commands@{ip}",
                )
                raise_text = (
                    f"Raised `{type(err).__name__}` while running init_commands."
                )
            else:
                if exit_status == 0:
                    log.info(
                        f"Cluster init commands executed successfully!",
                        detail=f"init_commands@{ip}",
                    )
                else:
                    log.error(
                        f"Failed running command `{command}`: ```\n{stderr_text}\n```",
                        detail=f"init_commands@{ip}",
                    )
                    raise_text = f"Bad exit_code from command `{command}`"

            ssh.close()
            if stdout_text != "":
                log.debug(
                    text=f"STDOUT: ```\n{stdout_text}\n```",
                    detail=f"init_commands@{ip}",
                )
            if raise_text != "":
                raise ClusterInitError(raise_text)

    async def repair(self):
        if self.is_healthy():
            return

        old_ips = self.node_ips

        terraform_ready = False
        while not terraform_ready:
            log.debug(
                f"Waiting for Terraform to refresh its state...",
                detail="cluster repair",
            )
            time.sleep(5)
            terraform_refresh(verbose=True)
            time.sleep(5)

            new_ips = get_running_nodes_ips(cluster=self)
            log.debug(f"Old cluster IPs = {self.node_ips}")
            log.debug(f"New cluster IPs = {new_ips}")
            if len(new_ips) == len(self.node_ips):
                self.node_ips = new_ips
                await self.save()
                log.info("Bad Cluster Nodes are now respawned!")
                terraform_ready = True
            else:
                log.warning(
                    f"Replacement Node didn't spawn, retrying application of Terraform plans...",
                    detail="cluster repair",
                )
                time.sleep(5)

        log.debug("Wait for new nodes to get ready for running Tasks...")
        time.sleep(30)

        if self.is_healthy():
            new_nodes_ips = [ip for ip in new_ips if ip not in old_ips]
            # Reconnect destroyed nodes to EFS, if required:
            if self.use_efs:
                self.setup_efs(ip_list_to_run=new_nodes_ips, wait_time=0)
            # Re-run init commands in the new node:
            self.run_init_commands(ip_list_to_run=new_nodes_ips)
        else:
            raise ClusterInitError("Failed repairing Cluster")


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

    # Create new Cluster record:
    cluster = await Cluster.create(**filtered_cluster_data)
    return cluster


async def fetch_latest_online_cluster() -> QuerySet[Cluster]:
    latest_cluster = (
        await Cluster.filter(is_online=True).order_by("-created_at").first()
    )
    if latest_cluster:
        return latest_cluster
    else:
        raise Exception("No online clusters available.")
