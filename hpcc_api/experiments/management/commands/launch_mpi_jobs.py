import json
import sys
import time

from django.core.management.base import BaseCommand

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.clusters.management.commands.create_cluster import create_cluster
from hpcc_api.clusters.management.commands.destroy_cluster import destroy_cluster
from hpcc_api.utils.files import load_yaml, delete_remote_folder_over_ssh, transfer_folder_over_ssh
from hpcc_api.utils.process import launch_over_ssh


class Command(BaseCommand):
    help = "Run an MPI workload."

    def add_arguments(self, parser):
        parser.add_argument("--cluster-config-id", type=str)

    def print_success(self, message):
        self.stdout.write(self.style.SUCCESS(message))

    def print_error(self, message):
        self.stdout.write(self.style.ERROR(message))

    def handle(self, *args, **options):
        cluster_config_id = options["cluster_config_id"]

        try:
            mpi_run_yaml = load_yaml("./mpi_run.yaml")

            cluster_config = ClusterConfiguration.objects.get(
                label=cluster_config_id,
            )
            ip = cluster_config.entrypoint_ip
            user = cluster_config.username

            mpi_jobs_logs = []
            mpi_jobs = mpi_run_yaml["mpi_jobs"]

            # Compile target application
            self.print_success(f"{len(mpi_jobs)} MPI jobs read from `mpi_run.yaml`, starting...")

            for n, mpi_job in enumerate(mpi_jobs):
                self.print_success(f"Starting  MPI job {n+1} of {len(mpi_jobs)}: {mpi_job['label']}")

                # Make sure cluster is healthy before launching MPI job
                if n > 0:
                    self.print_success("Checking Cluster health...")
                    time.sleep(25)  # wait while the provider updates the terraform state

                # Start setup timer
                setup_start = time.time()

                # Restore cluster and re-copy my_files
                create_cluster(cluster_config)
                shared_dir_path = None
                if cluster_config.fsx:
                    shared_dir_path = "/fsx"
                elif cluster_config.nfs:
                    shared_dir_path = "/var/nfs_dir"
                if shared_dir_path is not None:
                    delete_remote_folder_over_ssh(
                        remote_folder_path=f"{shared_dir_path}/my_files",
                        ip=ip,
                        user=user,
                    )
                    transfer_folder_over_ssh(
                        local_folder_path="./my_files",
                        remote_destination_path=shared_dir_path,
                        ip=ip,
                        user=user,
                    )

                setup_status = launch_over_ssh(mpi_job['setup_command'], ip=ip, user=user, track_output=True)
                setup_end = time.time()
                setup_dt = setup_end - setup_start

                run_failures = 0
                run_start = time.time()
                run_status = launch_over_ssh(mpi_job['run_command'], ip=ip, user=user, track_output=True)

                if run_status == 0:  # successful execution in first try
                    run_end = time.time()
                    run_dt = run_end - run_start
                    run_failures = 1
                elif mpi_job.get("restore_command") is not None:
                    self.print_error("Failure occurred during MPI Job! Running restore command...")
                    # Retry until success or after maximum retries are reached
                    while run_status != 0 and run_failures < mpi_job['maximum_retries']:
                        time.sleep(25)  # wait while the provider updates the terraform state

                        # Make sure cluster is healthy before launching MPI job
                        self.print_success("Checking Cluster health...")
                        create_cluster(cluster_config)

                        run_status = launch_over_ssh(mpi_job['restore_command'], ip=ip, user=user, track_output=True)
                        if run_status != 0:
                            run_failures += 1  # increase failure counter
                    run_end = time.time()
                    run_dt = run_end - run_start
                else:
                    run_end = time.time()
                    run_dt = run_end - run_start
                    run_status = -1
                    run_failures = 1

                # Append timing results
                mpi_jobs_logs.append(
                    {
                        "label": mpi_job["label"],
                        "setup_status": "SUCCESS" if setup_status == 0 else "FAILURE",
                        "setup_duration": setup_dt,
                        "run_status": "SUCCESS" if run_status == 0 else "FAILURE",
                        "run_duration": run_dt,
                        "run_failures": run_failures,
                    }
                )

        except Exception as error:
            self.print_error(f"CommandError: {error}")
            sys.exit(1)

        else:            
            self.print_success(f"Successfully executed {len(mpi_jobs)} MPI jobs defined in `mpi_run.yaml`.")
            for n, log in enumerate(mpi_jobs_logs):
                print(json.dumps(log, indent=4))

        finally:
            if mpi_run_yaml["delete_cluster_after"]:
                destroy_cluster()
            else:
                self.print_error(f"\n !!! Remember to destroy your Cluster !!!\n")
