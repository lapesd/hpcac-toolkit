from datetime import datetime
import json
import sys
import time

from django.core.management.base import BaseCommand
from django.utils import timezone

from hpcatcloud.clusters.models import ClusterConfiguration
from hpcatcloud.clusters.management.commands.create_cluster import create_cluster
from hpcatcloud.clusters.management.commands.destroy_cluster import destroy_cluster
from hpcatcloud.experiments.models import MPIExperiment
from hpcatcloud.utils.files import load_yaml, delete_remote_folder_over_ssh, transfer_folder_over_ssh, download_experiment_results
from hpcatcloud.utils.processes import launch_over_ssh
from hpcatcloud.utils.timers import ExecutionTimer

class Command(BaseCommand):
    help = "Run an MPI workload."

    def add_arguments(self, parser):
        parser.add_argument("--cluster-config-id", type=str)

    def print_success(self, message):
        self.stdout.write(self.style.SUCCESS(message))

    def print_error(self, message):
        self.stdout.write(self.style.ERROR(message))

    def update_mpi_job_experiment_record(self, job_db_record, log):
        job_db_record.completed_at = timezone.now()
        job_db_record.number_of_failures = log["retries"]
        job_db_record.job_successfully_completed = True if "SUCCESS" in log["pipeline_steps"]["job_execution"] else False
        job_db_record.time_spent_checkpointing = log["timers"]["time_spent_checkpointing"]
        job_db_record.time_spent_executing = log["timers"]["time_spent_executing"]
        job_db_record.time_spent_restoring_cluster = log["timers"]["time_spent_restoring_cluster"]
        job_db_record.time_spent_setting_up_job = log["timers"]["time_spent_setting_up_job"]
        job_db_record.time_spent_spawning_cluster = log["timers"]["time_spent_spawning_cluster"]
        job_db_record.total_time_spent = log["timers"]["total_time_spent"]
        job_db_record.save()

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
                job_db_record = MPIExperiment.objects.create(
                    label=mpi_job["experiment_label"],
                    cluster_size=cluster_config.nodes,
                    cluster_has_efa=cluster_config.efa,
                    cluster_has_fsx=cluster_config.fsx,
                    cluster_is_ephemeral=cluster_config.transient,
                    cluster_instance_type=cluster_config.instance_type,
                    ft_technology=mpi_job["fault_tolerance_technology_label"],
                    ckpt_strategy=mpi_job["checkpoint_strategy_label"],
                )

                self.print_success(f"Starting  MPI job {n+1} of {len(mpi_jobs)}: {mpi_job['experiment_label']}")

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
                        "experiment_label": mpi_job["experiment_label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "FAILURE, error setting up MPI job.",
                            "job_execution": "FAILURE, error setting up MPI job.",
                        },
                        "timers": {
                            "time_spent_spawning_cluster": cluster_config.spawn_time,
                            "time_spent_setting_up_job": job_setup_timer.get_elapsed_time(),
                            "time_spent_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "time_spent_restoring_cluster": restoring_timer.get_elapsed_time(),
                            "time_spent_executing": exec_timer.get_elapsed_time(),
                        },
                        "retries": 0
                    }
                    log["timers"]["total_time_spent"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    self.update_mpi_job_experiment_record(job_db_record, log)
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
                        "experiment_label": mpi_job["experiment_label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "SUCCESS!",
                            "job_execution": "SUCCESS!",
                        },
                        "timers": {
                            "time_spent_spawning_cluster": cluster_config.spawn_time,
                            "time_spent_setting_up_job": job_setup_timer.get_elapsed_time(),
                            "time_spent_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "time_spent_restoring_cluster": restoring_timer.get_elapsed_time(),
                            "time_spent_executing": exec_timer.get_elapsed_time(),
                        },
                        "retries": retries,
                    }
                    log["timers"]["total_time_spent"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    self.update_mpi_job_experiment_record(job_db_record, log)
                    download_experiment_results(
                        remote_folder_path=mpi_job["remote_outputs_dir"],
                        local_destination_path=f"{timezone.now().strftime('%d-%m-%Y_%H-%M-%S')}-{mpi_job['experiment_label']}".replace(" ", "_").lower(),
                        ip=ip,
                        user=user,
                    )
                    continue

                # If failure happens and there's no restore command available, return failure
                if mpi_job.get("restore_command") is None:
                    log = {
                        "experiment_label": mpi_job["experiment_label"],
                        "pipeline_steps": {
                            "cluster_health_check": "SUCCESS!",
                            "job_setup": "SUCCESS!",
                            "job_execution": "FAILURE, no restore command available!",
                        },
                        "timers": {
                            "time_spent_spawning_cluster": cluster_config.spawn_time,
                            "time_spent_setting_up_job": job_setup_timer.get_elapsed_time(),
                            "time_spent_checkpointing": checkpointing_timer.get_elapsed_time(),
                            "time_spent_restoring_cluster": restoring_timer.get_elapsed_time(),
                            "time_spent_executing": exec_timer.get_elapsed_time(),
                        },
                        "retries": retries
                    }
                    log["timers"]["total_time_spent"] = sum(log["timers"].values())
                    mpi_jobs_logs.append(log)
                    self.update_mpi_job_experiment_record(job_db_record, log)
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
                    "experiment_label": mpi_job["experiment_label"],
                    "pipeline_steps": {
                        "cluster_health_check": "SUCCESS!",
                        "job_setup": "SUCCESS!",
                        "job_execution": "SUCCESS!" if run_status == 0 else "FAILURE.",
                    },
                    "timers": {
                        "time_spent_spawning_cluster": cluster_config.spawn_time,
                        "time_spent_setting_up_job": job_setup_timer.get_elapsed_time(),
                        "time_spent_checkpointing": checkpointing_timer.get_elapsed_time(),
                        "time_spent_restoring_cluster": restoring_timer.get_elapsed_time(),
                        "time_spent_executing": exec_timer.get_elapsed_time(),
                    },
                    "retries": retries,
                }
                log["timers"]["total_time_spent"] = sum(log["timers"].values())
                self.update_mpi_job_experiment_record(job_db_record, log)
                download_experiment_results(
                    remote_folder_path=mpi_job["remote_outputs_dir"],
                    local_destination_path=f"{timezone.now().strftime('%d-%m-%Y_%H-%M-%S')}-{mpi_job['experiment_label']}".replace(" ", "_").lower(),
                    ip=ip,
                    user=user,
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
