import subprocess
import sys
import time

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
        parser.add_argument(
            "--np",
            type=int,
            help="Number of MPI ranks to be used",
        )

    def handle(self, *args, **options):
        cluster_config_id = options["cluster_config_id"]

        try:
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
            print(f"\t Total Nodes: {cluster_config.nodes}")
            print(f"\t Total vCPU cores: {cluster_config.vcpus}")
            ppn = round(cluster_config.vcpus/cluster_config.nodes)
            print(f"\t Maximum MPI ranks per node: {ppn}")

            # Generate hostfile
            self.stdout.write(
                self.style.SUCCESS("Generating hostfile...")
            )
            base_ip = "10.0.0."
            with open("./my_files/hostfile", "w") as file:
                for i in range(10, 10 + cluster_config.nodes):
                    file.write(f"{base_ip}{i} slots={ppn}\n")

            # Copy everything inside `my_files` to the NFS dir inside the Cluster
            subprocess.run(
                [
                    "scp",
                    "-r",
                    f"./",
                    f"{user}@{ip}:/var/nfs_dir/dynemol",
                ],
                cwd=f"./my_files",
                check=True,
            )

            """
            workloads = ["small", "medium", "large"]
            nodes = [1, 2, 3, 4]
            hosts = [
                "10.0.0.10",
                "10.0.0.10,10.0.0.11",
                "10.0.0.10,10.0.0.11,10.0.0.12",
                "10.0.0.10,10.0.0.11,10.0.0.12,10.0.0.13",
            ]

            for workload in workloads:
                # Copy experiment files to NFS dir inside the Cloud Cluster
                subprocess.run(
                    [
                        "scp",
                        "-r",
                        f"./{workload}",
                        f"{user}@{ip}:/var/nfs_dir/dynemol",
                    ],
                    cwd=f"../sample_workloads/dynemol",
                    check=True,
                )

                for n in nodes:
                    # Start time
                    start_time = time.time()

                    subprocess.run(
                        [
                            "ssh",
                            f"{user}@{ip}",
                            f"cd /var/nfs_dir/dynemol && mpiexec.hydra \
                                -genv OMP_NUM_THREADS=8 \
                                -n {n*4} \
                                -ppn 4 \
                                -hosts {hosts[n-1]} /home/ec2-user/Dynemol/dynemol",
                        ],
                        check=True,
                    )

                    # Record the end time
                    end_time = time.time()

                    # Compute the total duration
                    duration = end_time - start_time

                    with open("spot-ebs-efa.txt", "a") as file:
                        file.write(
                            f"Workload {workload} with {n} nodes took {duration} to complete.\n"
                        )

                # Remove experiment files
                subprocess.run(
                    [
                        "ssh",
                        f"{user}@{ip}",
                        f"cd /var/nfs_dir && rm -r ./dynemol",
                    ],
                    check=True,
                )
            """

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        else:
            self.stdout.write(
                self.style.SUCCESS(f"Successfully executed the experiments.")
            )
            self.stdout.write(
                self.style.ERROR(f"\n !!! Remember to destroy your Cluster !!!\n")
            )
