from datetime import datetime

from hpcac_cli.models.cluster import (
    insert_cluster_record,
    fetch_latest_online_cluster,
)
from hpcac_cli.utils.chronometer import Chronometer
from hpcac_cli.utils.logger import Logger
from hpcac_cli.utils.parser import parse_yaml
from hpcac_cli.utils.providers.aws import (
    get_instance_type_details,
    get_cluster_nodes_ip_addresses,
)
from hpcac_cli.utils.terraform import (
    TF_DIR,
    generate_cluster_tfvars_file,
    save_cluster_terraform_files,
    get_cluster_terraform_files,
    terraform_init,
    terraform_apply,
    terraform_destroy,
)


log = Logger()


async def create_cluster():
    log.info("Invoked `create-cluster` command...")
    log.info("Parsing contents of `cluster_config.yaml` file...")
    cluster_config = parse_yaml("cluster_config.yaml")
    for key, value in cluster_config.items():
        log.debug(text=f"{key}: {value}")
    log.info("Parsed `cluster_config.yaml` file successfully!")

    # Auto-generate cluster_tag:
    log.info("Generating `cluster_tag`...")
    provider = cluster_config["provider"]
    az = cluster_config["availability_zone"]
    node_count = cluster_config["node_count"]
    instance_type = cluster_config["node_instance_type"]
    # ami_id = cluster_config["node_ami"]
    use_spot = cluster_config["use_spot"]
    use_efs = cluster_config["use_efs"]
    timestamp = datetime.now().strftime("%H-%M-%S--%d-%m-%Y")

    cluster_tag = f"{provider}-{az}-{node_count}x-{instance_type}{'-spot' if use_spot else ''}{'-efs' if use_efs else ''}-{timestamp}"

    cluster_config["cluster_tag"] = cluster_tag
    log.info(
        text=f"Cluster will be created with tag=`{cluster_config['cluster_tag']}`!"
    )

    # Check if there is already a cluster online and mark it as offline
    log.info(f"Checking existing online Clusters...")
    try:
        cluster = await fetch_latest_online_cluster()
    except:
        log.info(text="No previously online Clusters.")
    else:
        cluster.is_online = False
        await cluster.save()
        log.warning(
            text=f"An existing online Cluster will be overwritten by your new configuration."
        )

    # Start Chronometer
    cluster_spawn_chronometer = Chronometer()
    cluster_spawn_chronometer.start()

    # Generate terraform.tfvars file:
    log.info(f"Generating Terraform configuration...")
    generate_cluster_tfvars_file(cluster_config=cluster_config)
    log.info(f"Generated Terraform configuration!")

    # Copy cloud blueprints and save terraform files in a MinIO bucket:
    log.info(f"Saving Terraform blueprints in MinIO...")
    save_cluster_terraform_files(cluster_config=cluster_config)
    log.info(f"Saved Terraform blueprints in MinIO!")

    # Get cluster details from provider:
    log.info(f"Getting Cluster details from `{provider}` provider...")
    instance_details = await get_instance_type_details(
        cluster_config["node_instance_type"]
    )
    for key, value in instance_details.items():
        log.debug(text=f"{key}: {value}", detail="get_instance_type_details")
    log.info(f"Received Cluster details from `{provider}` provider!")

    # Update cluster Postgres record:
    log.info(f"Updating Cluster record in Postgres...")
    cluster_config.update(instance_details)  # add instance details keys
    cluster_config.update({"node_ips": []})  # add empty ip address list
    cluster = await insert_cluster_record(cluster_data=cluster_config)
    log.info(f"Updated Cluster record in Postgres!")

    # Move terraform files to TF_DIR:
    log.info(f"Downloading Cluster Terraform blueprints to `{TF_DIR}`...")
    get_cluster_terraform_files(cluster_config=cluster_config)
    log.info(f"Terraform files saved at `{TF_DIR}`!")

    # Terraform init and apply
    log.info(f"Initializing Terraform...")
    terraform_init()
    log.info("Terraform is initialized!")

    log.info(f"Applying Terraform plans...")
    terraform_apply(verbose=True, retry=True)
    log.info(f"Terraform plans were applied!")

    # Update cluster Postgres record:
    log.info(f"Updating Cluster record in Postgres...")
    cluster.is_online = True
    cluster.node_ips = get_cluster_nodes_ip_addresses(
        cluster=cluster,
        number_of_nodes=cluster.node_count,
        region=cluster.region,
    )
    await cluster.save()
    log.info(f"Updated Cluster record in Postgres!")

    # Setup EFS in all nodes:
    if cluster.use_efs:
        log.info("Setting up Cluster EFS...")
        cluster.setup_efs(ip_list_to_run=cluster.node_ips)
    log.info("Cluster EFS setup is complete!")

    # Run cluster init commands:
    log.info("Running Cluster Nodes initialization commands...")
    cluster.run_init_commands(ip_list_to_run=cluster.node_ips)
    log.info("Completed Cluster Nodes initialization!")

    # Stop Chronometer and save Cluster spawn time
    cluster_spawn_chronometer.stop()
    cluster.time_spent_spawning_cluster = cluster_spawn_chronometer.get_elapsed_time()
    await cluster.save()

    log.info("Your cluster is ready! Remember to destroy it after using!!!")
    log.info(f"Your Cluster public IP addresses: {cluster.node_ips}")


async def destroy_cluster():
    log.info("Destroying cluster...")
    latest_cluster = await fetch_latest_online_cluster()
    latest_cluster.is_online = False
    await latest_cluster.save()
    terraform_destroy(verbose=True)
