import json
import sys
import time

from django.core.management.base import BaseCommand

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.clusters.management.commands.create_cluster import create_cluster
from hpcc_api.clusters.management.commands.destroy_cluster import destroy_cluster
from hpcc_api.utils.files import load_yaml, delete_remote_folder_over_ssh, transfer_folder_over_ssh
from hpcc_api.utils.processes import launch_over_ssh
from hpcc_api.utils.timers import ExecutionTimer

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

                job_setup_timer = ExecutionTimer()
                checkpointing_timer = ExecutionTimer()
                restoring_timer = ExecutionTimer()
                exec_timer = ExecutionTimer()
                
                # Make sure cluster is healthy before launching MPI job
                job_setup_timer.start()
                if n > 0:
                    job_setup_timer.stop()
                    self.print_success("Checking Cluster health...")
                    time.sleep(25)  # wait while the provider updates the terraform state
                    job_setup_timer.resume()
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
                job_setup_timer.stop()

                if setup_status != 0:
                    log = {
                        "label": mpi_job["label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "SUCCESS!",
                            "job_execution": "SUCCESS!",
                        },
                        "timers": {
                            "cluster_spawn": cluster_config.spawn_time,
                            "job_setup": job_setup_timer.get_elapsed_time(),
                            "system_wide_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "cluster_restoring": restoring_timer.get_elapsed_time(),
                            "execution": exec_timer.get_elapsed_time(),
                        },
                        "retries": 0
                    }
                    log["timers"]["total"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    continue

                # Start execution
                retries = 0
                run_status = -1
                exec_timer.start()
                run_status = launch_over_ssh(
                    mpi_job['run_command'], 
                    ip=ip, 
                    user=user, 
                    track_output=True
                )
                exec_timer.stop()
                if run_status == 0:
                    log = {
                        "label": mpi_job["label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "SUCCESS!",
                            "job_execution": "SUCCESS!",
                        },
                        "timers": {
                            "cluster_spawn": cluster_config.spawn_time,
                            "job_setup": job_setup_timer.get_elapsed_time(),
                            "system_wide_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "cluster_restoring": restoring_timer.get_elapsed_time(),
                            "execution": exec_timer.get_elapsed_time(),
                        },
                        "retries": retries,
                    }
                    log["timers"]["total"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    continue

                # If no restore command available, return failure
                if mpi_job.get("restore_command") is None:
                    log = {
                        "label": mpi_job["label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "SUCCESS!",
                            "job_execution": "FAILURE, no restore command available!",
                        },
                        "timers": {
                            "cluster_spawn": cluster_config.spawn_time,
                            "job_setup": job_setup_timer.get_elapsed_time(),
                            "system_wide_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "cluster_restoring": restoring_timer.get_elapsed_time(),
                            "execution": exec_timer.get_elapsed_time(),
                        },
                        "retries": retries
                    }
                    log["timers"]["total"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    continue

                # If first run fails, retry until success or maximum_retries reached
                while run_status != 0 and retries < mpi_job['maximum_retries']:
                    self.print_error("Failure occurred during MPI Job! Running restore command...")
                    retries += 1
                    restoring_timer.start()
                    # wait while the provider updates the terraform state
                    time.sleep(15)
                    self.print_success("Checking Cluster health...")
                    create_cluster(cluster_config)
                    restoring_timer.stop()

                    exec_timer.start()
                    run_status = launch_over_ssh(mpi_job['restore_command'], ip=ip, user=user, track_output=True)
                    exec_timer.stop()

                # Append timing results
                log = {
                    "label": mpi_job["label"],
                    "pipeline_steps": {
                        "cluster_health_check": "SUCCESS!",
                        "job_setup": "SUCCESS!",
                        "job_execution": "SUCCESS!" if run_status == 0 else "FAILURE.",
                    },
                    "timers": {
                        "cluster_spawn": cluster_config.spawn_time,
                        "job_setup": job_setup_timer.get_elapsed_time(),
                        "system_wide_checkpointing": checkpointing_timer.get_elapsed_time(),
                        "cluster_restoring": restoring_timer.get_elapsed_time(),
                        "execution": exec_timer.get_elapsed_time(),
                    },
                    "retries": retries,
                }
                log["timers"]["total"] = sum(log["timers"].values())
                mpi_jobs_logs.append(log)

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
