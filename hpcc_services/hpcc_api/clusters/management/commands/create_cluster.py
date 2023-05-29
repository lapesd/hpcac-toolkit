import os
import re
import subprocess
import sys

from django.core.management.base import BaseCommand
from minio import Minio

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.exceptions import ConfigurationError


def spawn_cluster(cluster_config: ClusterConfiguration) -> str:
    # Get ClusterConfiguration blueprint files from the MinIO bucket
    minio = Minio(
        "localhost:9000",
        access_key="root",
        secret_key="password",
        secure=False,
    )

    file_names = ["versions.tf", "provider.tf", "cluster.tf", "terraform.tfvars"]
    tf_dir = "./tmp_tf_dir"
    for file_name in file_names:
        minio_response = minio.fget_object(
            cluster_config.minio_bucket_name,
            file_name,
            os.path.abspath(f"{tf_dir}/{file_name}"),
        )
        print(
            "Downloaded `{0}` object with etag: `{1}` from bucket `{2}`".format(
                minio_response.object_name,
                minio_response.etag,
                cluster_config.minio_bucket_name,
            )
        )

    # Initialize Terraform
    subprocess.run(["terraform", "init"], cwd=tf_dir, check=True)

    # Apply the Terraform configuration, destroying dangling resources in case of failure
    try:
        process = subprocess.Popen(
            ["terraform", "apply", "-auto-approve"],
            cwd=tf_dir,
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

    except subprocess.CalledProcessError:
        subprocess.run(
            ["terraform", "destroy", "-auto-approve"], cwd=tf_dir, check=True
        )
        raise ConfigurationError(
            f"Failed spawning a cluster based on ClusterConfig `{cluster_config}`!"
        )


class Command(BaseCommand):
    help = "Spawns a Cluster from a previously created ClusterConfiguration."

    def add_arguments(self, parser):
        parser.add_argument(
            "config_label", type=str, help="The ClusterConfiguration label"
        )

    def handle(self, *args, **options):
        config_label = options["config_label"]

        try:
            cluster_config = ClusterConfiguration.objects.get(label=config_label)
            spawn_cluster(cluster_config)

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        else:
            self.stdout.write(
                self.style.SUCCESS(
                    f"Successfully spawned Cluster `{cluster_config.label}`!"
                )
            )
