import os
import subprocess
import sys

from django.core.management.base import BaseCommand

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.clusters.management.commands.create_cluster import spawn_cluster
from hpcc_api.clusters.management.commands.create_cluster_config import (
    generate_cluster_blueprint_from_yaml_definitions,
)
from hpcc_api.exceptions import ConfigurationError


class Command(BaseCommand):
    help = "Fetch experiment source code from GitHub, create a cluster and runs an experiment."

    def add_arguments(self, parser):
        parser.add_argument(
            "--experiment-path",
            type=str,
            help="The local path containing the experiment source code",
        )
        parser.add_argument(
            "--github-repository",
            type=str,
            help="The GitHub repository containing the experiment source code",
        )

    def handle(self, *args, **options):
        experiment_path = options["experiment_path"]
        github_repository = options["github_repository"]

        try:
            if experiment_path is not None:
                app_directory = experiment_path
            elif github_repository is not None:
                raise NotImplementedError(f"GitHub support is not implemented yet")
            else:
                raise ConfigurationError(
                    f"Please provide an --experiment-path or a --github-repository argument"
                )

            cluster_config = generate_cluster_blueprint_from_yaml_definitions(
                app_directory
            )
            master_node_ip = spawn_cluster(cluster_config)
            
            # Copy experiment files to NFS dir inside the Cloud Cluster
            subprocess.run(["scp", "-r", "./", f"ec2-users@{master_node_ip}:/var/nfs_dir"], cwd=app_directory, check=True)
            
            # Compile experiment application
            subprocess.run(["ssh", f"ec2-users@{master_node_ip}", "make", "all", "-C", "/var/nfs_dir/jacobi"], cwd=app_directory, check=True)

            """
            build-jacobi-ulfm: ## compile a fault-tolerant (ULFM) Jacobi solver
                scp -r ../../sample_apps/jacobi ec2-user@$(ip):/var/nfs_dir
                ssh ec2-user@$(ip) make all -C /var/nfs_dir/jacobi
                scp -r ./hostfile ec2-user@$(ip):/var/nfs_dir

            run-jacobi-ulfm: build-jacobi-ulfm ## execute a fault-tolerant (ULFM) Jacobi solver
                time ssh ec2-user@$(ip) mpirun --oversubscribe --with-ft ulfm -np $(n) --hostfile /var/nfs_dir/hostfile /var/nfs_dir/jacobi/jacobi_ulfm -p 2 -q 2 -NB 128
            """

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        else:
            self.stdout.write(
                self.style.SUCCESS(f"Successfully executed the experiment!")
            )
