import os
import subprocess
import sys
import time
import yaml

from django.core.management.base import BaseCommand

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.clusters.management.commands.destroy_cluster import destroy_cluster


VM_USER_MAPPING = {
    "aws": "ec2-user",
    "vultr": "root",
}


class Command(BaseCommand):
    help = "Run an MPI workload."

    def add_arguments(self, parser):
        parser.add_argument(
            "--cluster-config-id",
            type=str,
            help="The ID of the ClusterConfiguration to be used",
        )

    def handle(self, *args, **options):
        cluster_config_id = options["cluster_config_id"]

        try:
            # Reading mpi_run.yaml file
            self.stdout.write(
                self.style.SUCCESS("Reading `mpi_run.yaml` information...")
            )
            # Ensure the input YAML file exists
            if not os.path.exists("./mpi_run.yaml"):
                raise FileNotFoundError(f"./mpi_run.yaml does not exist")
            # Read YAML definitions
            with open("./mpi_run.yaml", "r") as file:
                yaml_data = yaml.safe_load(file)


            # Get Cluster information
            self.stdout.write(
                self.style.SUCCESS("Getting cluster information...")
            )
            cluster_config = ClusterConfiguration.objects.get(
                label=cluster_config_id,
            )
            ip = cluster_config.entrypoint_ip
            user = cluster_config.username

            # Compile target application
            self.stdout.write(
                self.style.SUCCESS("Compiling target MPI application...")
            )
            source_dir = yaml_data["source_dir"]
            compile_command = yaml_data["compile_command"]
            remote_command = f"cd {source_dir} && {compile_command}"

            process = subprocess.Popen(
                ["ssh", f"{user}@{ip}", remote_command],
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


            # Execute MPI application
            remote_commands_durations = []
            remote_commands_list = yaml_data["execute_commands"]

            for remote_command in remote_commands_list:
                # Start time
                start_time = time.time()

                # Launch MPI workload
                process = subprocess.Popen(
                    ["ssh", f"{user}@{ip}", remote_command],
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

                # Record the end time
                end_time = time.time()

                # Compute the total duration
                duration = end_time - start_time
                remote_commands_durations.append(duration)

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        else:            
            self.stdout.write(
                self.style.SUCCESS(f"Successfully executed all commands defined in `mpi_run.yaml`.")
            )
            for ix, duration in enumerate(remote_commands_durations):
                print(f"Command: {remote_commands_list[ix]}")
                print(f"Execution time: {duration}\n")

        finally:
            if yaml_data["delete_cluster_after"]:
                destroy_cluster()
            else:
                self.stdout.write(
                    self.style.ERROR(f"\n !!! Remember to destroy your Cluster !!!\n")
                )
