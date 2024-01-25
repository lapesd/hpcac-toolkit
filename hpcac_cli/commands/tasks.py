from hpcac_cli.models.cluster import Cluster, fetch_latest_online_cluster
from hpcac_cli.models.task import Task, insert_task_record, is_task_tag_alredy_used

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
        cluster.clean_my_files()
        cluster.generate_hostfile(mpi_distribution="openmpi")
        cluster.upload_my_files()
        cluster.run_command(
            task.setup_command,
            ip_list_to_run=[
                cluster.node_ips[0]
            ],  # TODO: check if the node selection affects execution time
            raise_exception=True,
        )
        setup_task_chronometer.stop()
        log.info(f"Finished setup of Task `{task.task_tag}`!", detail=detail)

        # Execute Task:
        log.info(f"Starting executing Task `{task.task_tag}`...", detail=detail)
        execution_chronometer.start()
        try:
            cluster.run_command(
                task.run_command,
                ip_list_to_run=[
                    cluster.node_ips[0]
                ],  # TODO: check if the node selection affects execution time
                raise_exception=True,
            )
        except:
            log.warning(
                f"Exception while executing Task `{task.task_tag}`, aborting...",
                detail=detail,
            )
        else:
            successfully_executed_task = True
            log.info(
                f"Successfully finished execution of Task `{task.task_tag}`!",
                detail=detail,
            )
        finally:
            execution_chronometer.stop()

        # Start the retry loop:
        retries = task.retries_before_aborting
        failures_during_execution = 0
        for retry in range(1, retries + 1):
            if successfully_executed_task:
                break

            detail = f"retry {retry}"

            log.info(f"Repairing Cluster `{cluster.cluster_tag}`...", detail=detail)
            restoration_chronometer.start()
            await cluster.repair()
            restoration_chronometer.stop()
            log.info(
                f"Cluster `{cluster.cluster_tag}` repaired successfully!", detail=detail
            )

            log.info(f"Retrying execution of Task `{task.task_tag}`...", detail=detail)
            execution_chronometer.resume()
            try:
                cluster.run_command(
                    task.restart_command,
                    ip_list_to_run=[
                        cluster.node_ips[0]
                    ],  # TODO: check if the node selection affects execution time
                    raise_exception=True,
                )
            except:
                failures_during_execution += 1
                log.warning(
                    f"Exception while executing Task `{task.task_tag}`, aborting...",
                    detail=detail,
                )
            else:
                successfully_executed_task = True
                log.info(
                    f"Successfully finished execution of Task `{task.task_tag}`!",
                    detail=detail,
                )
            finally:
                execution_chronometer.stop()

        log.info(text=f"Starting download of Task results...", detail=detail)
        cluster.download_directory(
            remote_path=task.remote_outputs_dir, local_path=f"./results/{task.task_tag}"
        )
        log.info(text=f"Completed download of tasks results!", detail=detail)

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
