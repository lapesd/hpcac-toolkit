import subprocess
import sys

from django.core.management.base import BaseCommand

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.clusters.management.commands.destroy_cluster import destroy_cluster
from hpcc_api.clusters.management.commands.create_cluster import create_cluster
from hpcc_api.clusters.management.commands.create_cluster_config import (
    generate_cluster_blueprint_from_yaml_definitions,
)
from hpcc_api.exceptions import ConfigurationError


VM_USER_MAPPING = {
    "aws": "ec2-user",
    "aws-spot": "ec2-user",
    "vultr": "root",
}


class Command(BaseCommand):
    help = "Fetch experiment source code, create a cluster and runs an experiment."

    def add_arguments(self, parser):
        parser.add_argument(
            "--cluster-config-id",
            type=str,
            help="The id/label of the desired cluster configuration",
        )
        parser.add_argument(
            "--yaml-file",
            type=str,
            help="The path to the YAML file with the cluster configuration",
        )
        parser.add_argument(
            "--application-path",
            type=str,
            help="The local path containing the application source code",
        )
        parser.add_argument(
            "--github-repository",
            type=str,
            help="The GitHub repository containing the application source code",
        )

    def handle(self, *args, **options):
        cluster_config_id = options["cluster_config_id"]
        yaml_file = options["yaml_file"]
        application_path = options["application_path"]
        github_repository = options["github_repository"]

        try:
            raise NotImplementedError(
                "This command is a WIP and is currently not working correctly."
            )

            if application_path is not None:
                app_directory = application_path
            elif github_repository is not None:
                raise NotImplementedError(f"GitHub support is not implemented yet")
            else:
                raise ConfigurationError(
                    f"Please provide a --github-repository or an --application-path"
                )

            if cluster_config_id is not None:
                cluster_config = ClusterConfiguration.objects.get(
                    label=cluster_config_id
                )
            elif yaml_file is not None:
                cluster_config = generate_cluster_blueprint_from_yaml_definitions(
                    app_directory
                )
            else:
                raise ConfigurationError(
                    f"Please provide a --cluster-config-id or a --yaml-file"
                )

            # Launch Cloud Cluster
            master_node_ip = create_cluster(cluster_config)[0]
            vm_user = VM_USER_MAPPING.get(cluster_config.cloud_provider)

            # Copy experiment files to NFS dir inside the Cloud Cluster
            subprocess.run(
                [
                    "scp",
                    "-r",
                    "./",
                    f"{vm_user}@{master_node_ip}:/var/nfs_dir",
                ],
                cwd=app_directory,
                check=True,
            )

            # TODO implement remaining logic to execute the application

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            destroy_cluster()
            sys.exit(1)

        else:
            self.stdout.write(
                self.style.SUCCESS(f"Successfully executed the experiment!")
            )
