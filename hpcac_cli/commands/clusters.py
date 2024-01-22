from hpcac_cli.models.cluster import (
    insert_cluster_record,
    is_cluster_tag_alredy_used,
    fetch_latest_online_cluster,
)
from hpcac_cli.utils.chronometer import Chronometer
from hpcac_cli.utils.logger import info, print_map
from hpcac_cli.utils.parser import parse_yaml
from hpcac_cli.utils.prompt import prompt_text, prompt_confirmation
from hpcac_cli.utils.providers.aws import (
    get_instance_type_details,
    get_cluster_nodes_ip_addresses,
)
from hpcac_cli.utils.terraform import (
    generate_cluster_tfvars_file,
    save_cluster_terraform_files,
    get_cluster_terraform_files,
    terraform_init,
    terraform_apply,
    terraform_destroy,
    TF_DIR,
)


async def create_cluster():
    info("Reading `cluster_config.yaml`...")
    cluster_config = parse_yaml("cluster_config.yaml")
    print_map(cluster_config)

    # Prompt for cluster_tag:
    cluster_tag = ""
    cluster_tag = prompt_text(
        text="Enter a `cluster_tag` or CTRL+C to cancel cluster creation:"
    )
    while cluster_tag == "" or await is_cluster_tag_alredy_used(cluster_tag):
        if cluster_tag == "":
            cluster_tag = prompt_text(text="`cluster_tag` can't be empty:")
        if await is_cluster_tag_alredy_used(cluster_tag):
            cluster_tag = prompt_text(
                text=f"cluster_tag `{cluster_tag}` is already used, choose another one:"
            )
    cluster_config["cluster_tag"] = cluster_tag

    # Prompt for confirmation:
    continue_creation = prompt_confirmation(
        text=f"Confirm creation of Cluster: `{cluster_tag}`?"
    )
    if continue_creation:
        info(
            f"Attempting creation of Cluster `{cluster_tag}` at Cloud "
            f"Provider: `{cluster_config['provider']}`"
        )
    else:
        info("Cluster creation CANCELLED by the user.")
        return

    # Check if there is already a cluster online and mark it as offline
    try:
        cluster = await fetch_latest_online_cluster()
    except:
        pass
    else:
        cluster.is_online = False
        await cluster.save()

    # Start Chronometer
    cluster_spawn_chronometer = Chronometer()
    cluster_spawn_chronometer.start()

    # Generate terraform.tfvars file:
    info(f"Generating terraform.tfvars file for Cluster `{cluster_tag}`...")
    generate_cluster_tfvars_file(cluster_config=cluster_config)

    # Copy cloud blueprints and save terraform files in a MinIO bucket:
    info(f"Saving cloud blueprints in a MinIO bucket for Cluster `{cluster_tag}`...")
    save_cluster_terraform_files(cluster_config=cluster_config)

    # Save cluster_config to Postgres:
    instance_details = await get_instance_type_details(
        cluster_config["node_instance_type"]
    )
    cluster_config.update(instance_details)  # add instance details keys
    cluster_config.update({"node_ips": []})  # add empty ip address list
    cluster = await insert_cluster_record(cluster_data=cluster_config)

    # Download terraform files to TF_DIR:
    info(f"Downloading terraform blueprints for Cluster `{cluster_tag}`...")
    get_cluster_terraform_files(cluster_config=cluster_config)

    # Terraform init and apply
    terraform_init()
    terraform_apply()

    cluster.is_online = True
    cluster.node_ips = get_cluster_nodes_ip_addresses(
        cluster_tag=cluster.cluster_tag, region=cluster.region
    )
    await cluster.save()

    # Setup EFS in all nodes:
    if cluster.use_efs:
        cluster.setup_efs(ip_list_to_run=cluster.node_ips)

    # Run cluster init commands:
    for command in cluster_config["init_commands"]:
        cluster.run_command(command=command, ip_list_to_run=cluster.node_ips)

    # Stop Chronometer
    cluster_spawn_chronometer.stop()
    cluster.time_spent_spawning_cluster = cluster_spawn_chronometer.get_elapsed_time()
    await cluster.save()

    info("Your cluster is ready! Remember to destroy it after using!!!")
    info(f"Your Cluster public IP addresses: {cluster.node_ips}")


async def destroy_cluster():
    info("Destroying cluster...")
    latest_cluster = await fetch_latest_online_cluster()
    latest_cluster.is_online = False
    await latest_cluster.save()
    terraform_destroy()
