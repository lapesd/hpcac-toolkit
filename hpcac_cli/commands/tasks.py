import time

from hpcac_cli.models.cluster import Cluster, fetch_latest_online_cluster
from hpcac_cli.models.task import Task, insert_task_record, is_task_tag_alredy_used

from hpcac_cli.utils.chronometer import Chronometer
from hpcac_cli.utils.logger import error, info, print_map
from hpcac_cli.utils.parser import parse_yaml


async def run_tasks():
    # Parse tasks information from yaml file:
    info("Reading `tasks_config.yaml`...")
    tasks_config = parse_yaml("tasks_config.yaml")
    print_map(tasks_config)

    # Fetch latest cluster information from Postgres:
    cluster = await fetch_latest_online_cluster()
    info(f"Found latest Cluster `{cluster.cluster_tag}` configuration!")

    # Make sure tasks have unique tags, aborting if not:
    if not tasks_config["overwrite_tasks"]:
        for task_data in tasks_config["tasks"]:
            if await is_task_tag_alredy_used(task_tag=task_data["task_tag"]):
                raise Exception(
                    f"Task record `{task_data['task_tag']}` already exists!"
                )

    # Insert new task records:
    task_objects = []
    for task_data in tasks_config["tasks"]:
        task_data["cluster_id"] = cluster.cluster_tag
        task = await insert_task_record(
            task_data=task_data, overwrite=tasks_config["overwrite_tasks"]
        )
        task_objects.append(task)

    # Run tasks serially:
    for i, task in enumerate(task_objects):
        # Create chronometers for task:
        setup_task_chronometer = Chronometer()
        execution_chronometer = Chronometer()
        _checkpoint_chronometer = (
            Chronometer()
        )  # TODO: add logic for periodic/preemptive checkpointing (system-level)
        restoration_chronometer = Chronometer()
        total_execution_chronometer = Chronometer()
        total_execution_chronometer.start()

        # Setup task:
        setup_task_chronometer.start()
        # Re-upload my_files:
        cluster.clean_my_files()
        cluster.generate_hostfile(mpi_distribution="openmpi")
        cluster.upload_my_files()
        # Run Task setup command
        cluster.run_command(task.setup_command, raise_exception=True)
        setup_task_chronometer.stop()

        # Run Task:
        first_run = True
        completed = False
        failures_during_execution = 0
        while not completed and (
            failures_during_execution < task.retries_before_aborting
        ):
            try:
                # Check if Cluster is ready
                if not cluster.is_healthy():
                    # Repair Cluster
                    restoration_chronometer.start()
                    cluster.repair()
                    restoration_chronometer.stop()

                    # Setup task:
                    setup_task_chronometer.resume()
                    # Re-upload my_files:
                    cluster.clean_my_files()
                    cluster.generate_hostfile(mpi_distribution="openmpi")
                    cluster.upload_my_files()
                    # Run Task setup command
                    cluster.run_command(task.setup_command, raise_exception=True)
                    setup_task_chronometer.stop()

                if first_run:
                    # Execute run command:
                    execution_chronometer.start()
                    cluster.run_command(task.run_command, raise_exception=True)
                else:
                    # Execute restart command:
                    execution_chronometer.resume()
                    cluster.run_command(task.restart_command, raise_exception=True)

            except:
                failures_during_execution += 1
            else:
                completed = True
            finally:
                execution_chronometer.stop()

        # TODO Download task results

        task.time_spent_spawning_cluster = cluster.time_spent_spawning_cluster
        task.time_spent_setting_up_task = setup_task_chronometer.get_elapsed_time()
        task.time_spent_restoring_cluster = restoration_chronometer.get_elapsed_time()
        task.time_spent_executing_task = execution_chronometer.get_elapsed_time()
        task.time_spent_checkpointing = _checkpoint_chronometer.get_elapsed_time()
        await task.save()

        total_execution_chronometer.stop()

        if failures_during_execution >= task.retries_before_aborting:
            info(
                f"!!! Task `{task.task_tag}` aborted after {failures_during_execution} failures !!!"
            )
        else:
            info(
                f"!!! Task `{task.task_tag}` completed in {total_execution_chronometer.get_elapsed_time()} seconds !!!\n\n"
            )
