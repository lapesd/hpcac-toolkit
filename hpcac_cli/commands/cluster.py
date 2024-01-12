from hpcac_cli.models.cluster import upsert_cluster
from hpcac_cli.utils.logger import error, info, print_map
from hpcac_cli.utils.parser import parse_yaml
from hpcac_cli.utils.prompt import prompt_text, prompt_confirmation
from hpcac_cli.utils.providers.aws import get_instance_type_details
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

    try:
        # Prompt for cluster_tag:
        cluster_tag = ""
        cluster_tag = prompt_text(
            text="Enter a `cluster_tag` or CTRL+C to cancel cluster creation:"
        )
        while cluster_tag == "":
            cluster_tag = prompt_text(text="`cluster_tag` can't be empty:")
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
            error("Cluster creation CANCELLED by the user.")
            return

        # Generate terraform.tfvars file:
        info(f"Generating terraform.tfvars file for Cluster `{cluster_tag}`...")
        generate_cluster_tfvars_file(cluster_config=cluster_config)

        # Copy cloud blueprints and save terraform files in a MinIO bucket:
        info(
            f"Saving cloud blueprints in a MinIO bucket for Cluster `{cluster_tag}`..."
        )
        save_cluster_terraform_files(cluster_config=cluster_config)

        # Save cluster_config to Postgres:
        instance_details = await get_instance_type_details(
            cluster_config["node_instance_type"]
        )
        cluster_config.update(instance_details)  # add instance details keys
        await upsert_cluster(cluster_data=cluster_config)

        # Download terraform files to TF_DIR:
        info(f"Downloading terraform blueprints for Cluster `{cluster_tag}`...")
        get_cluster_terraform_files(cluster_config=cluster_config)

    except KeyboardInterrupt:
        error("\nCluster creation CANCELLED by the user.")

    try:
        # Terraform init and apply
        terraform_init()
        terraform_apply()

        info("Your cluster is ready! Remember to destroy it after using!!!")

    except Exception as e:
        terraform_destroy()
        raise Exception(f"Terraform subprocess error: {e}")


def destroy_cluster():
    info("Destroying cluster...")
    terraform_destroy()
