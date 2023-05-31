import os
import re
import subprocess
import textwrap

from django.core.management.base import BaseCommand
from minio import Minio

from hpcc_api.clusters.models import ClusterConfiguration


TF_DIR = "./tmp_tf_dir"


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
            "config_label", type=str, help="The ClusterConfiguration label"
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
            self.stdout.write(
                self.style.SUCCESS(
                    f"Successfully spawned a Cluster using the `{cluster_config.label} ClusterConfiguration`!"
                )
            )

            print(
                textwrap.dedent(
                    f"""
                To copy files from your machine to the cloud cluster, use the `scp` command:
                
                scp -r /Users/vanderlei/Code/lapesd/jacobi-method ec2-user@{master_node_ip[0]}:/var/nfs_dir

                The command above will copy the `jacobi-method` folder and all files inside it to the 
                /var/nfs_dir shared cluster directory.
    
                You can also execute commands in your cluster from your local machine using `ssh`:

                ssh ec2-user@{master_node_ip[0]} make all -C /var/nfs_dir/jacobi

                The command above will run the `make all -C /var/nfs_dir/jacobi` command, compiling the 
                jacobi-method application (https://github.com/vanderlei-filho/jacobi-method), which can 
                then be executed by the following:

                ssh ec2-user@{master_node_ip[0]} mpirun --oversubscribe --with-ft ulfm -np 4 --hostfile /var/nfs_dir/hostfile /var/nfs_dir/jacobi-method/jacobi_ulfm -p 2 -q 2 -NB 128

                Don't forget to edit an appropriate hostfile and copy it to the cluster.
                You can use the `hostfile.openmpi.example` and `hostfile.mvapich2.example` files as templates.

                Finally, if you want to access your cluster directly over the command-line, use SSH:
                
                ssh ec2-user@{master_node_ip[0]}
                """
                )
            )
