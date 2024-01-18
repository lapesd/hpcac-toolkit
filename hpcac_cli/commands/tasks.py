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
    for task_data in tasks_config["tasks"]:
        if await is_task_tag_alredy_used(task_tag=task_data["task_tag"]):
            raise Exception(f"Task record `{task_data['task_tag']}` already exists!")

    # Insert new task records:
    task_objects = []
    for task_data in tasks_config["tasks"]:
        task_data["cluster_id"] = cluster.cluster_tag
        task = await insert_task_record(task_data=task_data)
        task_objects.append(task)

    # Run tasks serially:
    for i, task in enumerate(task_objects):
        # Create chronometers for task:
        setup_chronometer = Chronometer()
        checkpoint_chronometer = Chronometer()
        restoration_chronometer = Chronometer()
        execution_chronometer = Chronometer()

        # Check that Cluster is ready
        # TODO: clear the /var/nfs_dir folder

        # Setup task:
        setup_chronometer.start()
        # TODO: copy files to /var/nfs_dir
        # TODO: run setup command
        setup_chronometer.stop()

        # Run task:
