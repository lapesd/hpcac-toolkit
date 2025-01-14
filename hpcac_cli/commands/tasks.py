import csv
import os
import time
from datetime import datetime

from hpcac_cli.models.cluster import fetch_latest_online_cluster
from hpcac_cli.models.task import (Task, TaskStatus, insert_task_record,
                                   is_task_tag_alredy_used)
from hpcac_cli.utils.chronometer import Chronometer
from hpcac_cli.utils.logger import Logger
from hpcac_cli.utils.parser import parse_yaml

log = Logger()


async def run_tasks():
    log.info("Invoked `run_tasks` command...")
    log.info("Parsing contents of `tasks_config.yaml` file...")
    tasks_config = parse_yaml("tasks_config.yaml")
    for key, value in tasks_config.items():
        log.debug(text=f"{key}: {value}")
    log.info("Parsed `tasks_config.yaml` successfully!")

    # Fetch latest cluster information from Postgres:
    log.info("Searching for existing online Clusters...")
    cluster = await fetch_latest_online_cluster()
    log.info(f"Found online Cluster `{cluster.cluster_tag}`!")

    # Make sure tasks have unique tags, aborting if not:
    if not tasks_config["overwrite_tasks"]:
        for task_data in tasks_config["tasks"]:
            if await is_task_tag_alredy_used(task_tag=task_data["task_tag"]):
                raise Exception(
                    f"Task record `{task_data['task_tag']}` already exists!"
                )

    # Insert new task records:
    log.info("Inserting new Task records in Postgres...")
    task_objects = []
    for task_data in tasks_config["tasks"]:
        task_data["cluster_id"] = cluster.cluster_tag
        task = await insert_task_record(
            task_data=task_data, overwrite=tasks_config["overwrite_tasks"]
        )
        task_objects.append(task)
    log.info("Inserted new Task records in Postgres!")

    # Run tasks serially:
    log.info("Starting Task loop...")
    for _, task in enumerate(task_objects):
        successfully_executed_task = False

        # Create chronometers for task:
        setup_task_chronometer = Chronometer()
        execution_chronometer = Chronometer()
        _checkpoint_chronometer = (
            Chronometer()
        )  # TODO: add logic for periodic/preemptive checkpointing (system-level)
        restoration_chronometer = Chronometer()
        total_execution_chronometer = Chronometer()
        total_execution_chronometer.start()

        # Setup Task:
        detail = "first attempt"
        log.info(f"Setting up Task {task.task_tag}...", detail=detail)
        setup_task_chronometer.start()
        cluster.clean_remote_my_files_directory()
        cluster.generate_hostfile(
            mpi_distribution="openmpi",
            nodes=task.nodes_to_use,
            slots_per_node=task.slots_per_node_to_use,
        )
        cluster.upload_my_files()
        setup_status = cluster.run_task(task.setup_commands)
        setup_task_chronometer.stop()
        if setup_status == TaskStatus.Success:
            log.info(f"Finished setup of Task `{task.task_tag}`!", detail=detail)
        else:
            log.error(f"Task setup failed, aborting...", detail=detail)
            exit(1)

        time.sleep(15)
        # Execute Task:
        log.info(f"Starting executing Task `{task.task_tag}`...", detail=detail)
        execution_chronometer.start()
        task_status = cluster.run_task(task.run_commands)
        execution_chronometer.stop()

        failures_during_execution = 0
        # Check TaskStatus:
        if task_status == TaskStatus.RemoteException:
            log.error(f"Task execution failed, aborting...", detail=detail)
            failures_during_execution += 1
            exit(1)
        elif task_status == TaskStatus.Success:
            log.info(f"Task {task.task_tag} completed!", detail=detail)
            successfully_executed_task = True
        elif task_status == TaskStatus.NodeEvicted:
            # Start the retry loop:
            failures_during_execution += 1
            retries = task.retries_before_aborting
            task_retry_status = TaskStatus.NotCompleted
            for retry in range(1, retries + 1):
                detail = f"retry {retry}"
                log.info(f"Repairing Cluster `{cluster.cluster_tag}`...", detail=detail)

                restoration_chronometer.start()
                await cluster.repair()
                restoration_chronometer.stop()
                log.info(
                    f"Cluster `{cluster.cluster_tag}` repaired successfully!",
                    detail=detail,
                )

                log.info(
                    f"Retrying execution of Task `{task.task_tag}`...", detail=detail
                )
                execution_chronometer.resume()
                task_retry_status = cluster.run_task(task.restart_command)
                execution_chronometer.stop()

                if task_retry_status == TaskStatus.Success:
                    log.info(f"Task {task.task_tag} completed!", detail=detail)
                    successfully_executed_task = True
                    break
                if task_retry_status == TaskStatus.RemoteException:
                    log.error(f"Task execution failed, aborting...", detail=detail)
                    exit(1)
        if task.remote_outputs_dir:
            log.info(text=f"Starting download of Task results...", detail=detail)
            cluster.download_directory(
                remote_path=task.remote_outputs_dir,
                local_path=f"./results/{task.task_tag}",
            )
            log.info(text=f"Completed download of tasks results!", detail=detail)

        task.completed_at = datetime.now()
        task.task_completed_successfully = successfully_executed_task
        task.time_spent_spawning_cluster = cluster.time_spent_spawning_cluster
        task.time_spent_setting_up_task = setup_task_chronometer.get_elapsed_time()
        task.time_spent_restoring_cluster = restoration_chronometer.get_elapsed_time()
        task.time_spent_executing_task = execution_chronometer.get_elapsed_time()
        task.time_spent_checkpointing = _checkpoint_chronometer.get_elapsed_time()
        await task.save()

        total_execution_chronometer.stop()

        if failures_during_execution > task.retries_before_aborting:
            log.error(
                f"!!! Task `{task.task_tag}` aborted after {failures_during_execution} failures !!!"
            )
        else:
            log.info(
                f"!!! Task `{task.task_tag}` completed in {total_execution_chronometer.get_elapsed_time()} seconds !!!\n\n"
            )


async def export_tasks():
    log.info("Invoked `export_tasks` command...")
    RESULTS_PATH = "./results"
    RESULTS_CSV_FILE_NAME = "task_results.csv"
    csv_file_path = os.path.join(RESULTS_PATH, RESULTS_CSV_FILE_NAME)

    # Ensure the results directory exists
    if not os.path.exists(RESULTS_PATH):
        os.makedirs(RESULTS_PATH)
        log.info(f"Created directory: {RESULTS_PATH}")

    # Remove existing CSV file if it exists
    if os.path.exists(csv_file_path):
        os.remove(csv_file_path)
        log.info(f"Removed existing file: {csv_file_path}")

    tasks = await Task.all()
    with open(csv_file_path, mode="w", newline="", encoding="utf-8") as file:
        writer = csv.writer(file)

        # Writing headers
        headers = [
            "task_tag",
            "cluster",
            "created_at",
            "started_at",
            "completed_at",
            "failures_during_execution",
            "retries_before_aborting",
            "fault_tolerance_technology_label",
            "checkpoint_strategy_label",
            "task_completed_successfully",
            "time_spent_spawning_cluster",
            "time_spent_setting_up_task",
            "time_spent_checkpointing",
            "time_spent_restoring_cluster",
            "time_spent_executing_task",
            "total_time_spent",
        ]
        writer.writerow(headers)

        # Writing rows
        for task in tasks:
            row = [
                task.task_tag,
                task.cluster_id,
                task.created_at,
                task.started_at,
                task.completed_at,
                task.failures_during_execution,
                task.retries_before_aborting,
                task.fault_tolerance_technology_label,
                task.checkpoint_strategy_label,
                task.task_completed_successfully,
                task.time_spent_spawning_cluster,
                task.time_spent_setting_up_task,
                task.time_spent_checkpointing,
                task.time_spent_restoring_cluster,
                task.time_spent_executing_task,
                task.time_spent_spawning_cluster
                + task.time_spent_checkpointing
                + task.time_spent_setting_up_task
                + task.time_spent_executing_task
                + task.time_spent_restoring_cluster,
            ]
            writer.writerow(row)

    log.info("Successfully exported results to `./results/task_results.csv` file!")
