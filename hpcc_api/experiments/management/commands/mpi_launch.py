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
            self.stdout.write(
                self.style.SUCCESS(f"Cluster `{cluster_config_id}` at `{cluster_config.cloud_provider}` provider:")
            )
            print(f"Total Nodes: {cluster_config.nodes}")
            print(f"Total vCPU cores: {cluster_config.vcpus}")
            ppn = round(cluster_config.vcpus/cluster_config.nodes)
            print(f"Maximum MPI ranks per node: {ppn}")


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


            # Copy everything inside `my_files` to the NFS dir inside the Cluster
            # Generate hostfile
            self.stdout.write(
                self.style.SUCCESS("Transfering `/my_files` to `/var/nfs_dir/`...")
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
            print("Done.")


            # Compile target application
            self.stdout.write(
                self.style.SUCCESS("Compiling target MPI application...")
            )
            source_dir = yaml_data["source_dir"]
            compile_command = yaml_data["compile_command"]
            remote_command = f"cd {source_dir} && {compile_command}"
            subprocess.run(
                ["ssh", f"{user}@{ip}", remote_command],
                check=True,
                shell=False,
                text=True,
            )
            print("Done.")


            # Execute MPI application
            np = yaml_data["np"]
            executable_path = yaml_data["executable_path"]
            remote_command = f"mpiexec -np {np} --hostfile /var/nfs_dir/my_files/hostfile {executable_path}"

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

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        else:            
            self.stdout.write(
                self.style.SUCCESS(f"Successfully executed the MPI workload. Duration: {duration}.")
            )

        finally:
            if yaml_data["delete_cluster_after"]:
                destroy_cluster()
            else:
                self.stdout.write(
                    self.style.ERROR(f"\n !!! Remember to destroy your Cluster !!!\n")
                )
