import os
import re
import subprocess
import textwrap

from django.core.management.base import BaseCommand
from minio import Minio

from hpcc_api.clusters.models import ClusterConfiguration


TF_DIR = "./tmp_terraform_dir"


def create_cluster(cluster_config: ClusterConfiguration) -> str:
    # Get ClusterConfiguration blueprint files from the MinIO bucket
    minio = Minio(
        "localhost:9000",
        access_key="root",
        secret_key="password",
        secure=False,
    )

    file_names = ["versions.tf", "provider.tf", "cluster.tf", "terraform.tfvars"]
    for file_name in file_names:
        minio_response = minio.fget_object(
            cluster_config.minio_bucket_name,
            file_name,
            os.path.abspath(f"{TF_DIR}/{file_name}"),
        )
        print(
            "Downloaded `{0}` object with etag: `{1}` from bucket `{2}`".format(
                minio_response.object_name,
                minio_response.etag,
                cluster_config.minio_bucket_name,
            )
        )

    # Initialize Terraform
    subprocess.run(["terraform", "init"], cwd=TF_DIR, check=True)

    # Apply the Terraform configuration, destroying dangling resources in case of failure
    process = subprocess.Popen(
        ["terraform", "apply", "-auto-approve"],
        cwd=TF_DIR,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )

    last_line = ""
    for line in iter(process.stdout.readline, ""):
        print(line, end="")
        last_line = line if line.strip() != "" else last_line

    process.stdout.close()
    process.wait()

    master_node_ip = re.findall(r"[0-9]+(?:\.[0-9]+){3}", last_line)

    return master_node_ip


class Command(BaseCommand):
    help = "Spawns a Cluster from a previously created ClusterConfiguration."

    def add_arguments(self, parser):
        parser.add_argument(
            "config_label",
            type=str,
            help="The ClusterConfiguration label",
        )

    def handle(self, *args, **options):
        config_label = options["config_label"]

        cluster_config = ClusterConfiguration.objects.get(label=config_label)
        master_node_ip = create_cluster(cluster_config)

        if len(master_node_ip) == 0:
            subprocess.run(
                ["terraform", "destroy", "-auto-approve"], cwd=TF_DIR, check=True
            )
            self.stdout.write(
                self.style.ERROR(
                    f"Failed spawning a cluster based on ClusterConfig `{cluster_config}`.\n"
                    "All resources DESTROYED!"
                )
            )
        else:
            # Update `entrypoint_ip`:
            cluster_config.entrypoint_ip = master_node_ip[0]
            cluster_config.save()

            ip = cluster_config.entrypoint_ip
            user = cluster_config.username

            self.stdout.write(
                self.style.SUCCESS(
                    f"Successfully spawned a Cluster using the `{cluster_config.label} ClusterConfiguration`!"
                )
            )

            ppn = round(cluster_config.vcpus/cluster_config.nodes)

            # Generate hostfile
            self.stdout.write(
                self.style.SUCCESS("Generating hostfile...")
            )
            base_ip = "10.0.0."
            hostfile_path = "./my_files/hostfile"
            if os.path.exists(hostfile_path):
                os.remove(hostfile_path)
            with open(hostfile_path, "w") as file:
                for i in range(10, 10 + cluster_config.nodes):
                    file.write(f"{base_ip}{i} slots={ppn}\n")
            print("Done.")

            # Copy everything inside `my_files` to the shared dir inside the Cluster
            shared_dir_path = None
            if cluster_config.fsx:
                shared_dir_path = "/fsx"
            elif cluster_config.nfs:
                shared_dir_path = "/var/nfs_dir"

            if shared_dir_path:
                self.stdout.write(
                    self.style.SUCCESS(f"Transfering `/my_files` to `{shared_dir_path}`...")
                )
                subprocess.run(
                    [
                        "scp",
                        "-r",
                        f"./",
                        f"{user}@{ip}:/var/nfs_dir/my_files",
                    ],
                    cwd=f"./my_files",
                    check=True,
                )
            else:
                self.stdout.write(
                    self.style.ERROR(f"Files in `/my_files` won't be transferred to the cluster (no shared directory)")
                )

            self.stdout.write(
                self.style.SUCCESS(
                    textwrap.dedent(
                        f"""                
                        To access your cluster over the command-line, use SSH:
                        ssh {user}@{ip}

                        Total Nodes: {cluster_config.nodes}
                        Total vCPU cores: {cluster_config.vcpus}
                        Maximum MPI ranks per node: {ppn}
                        """
                    )
                )
            )
