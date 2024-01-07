from hpcac_cli.utils.logger import error, info, print_map
from hpcac_cli.utils.parser import parse_yaml
from hpcac_cli.utils.prompt import prompt_text, prompt_confirmation
from hpcac_cli.utils.providers.aws import get_instance_types
from hpcac_cli.utils.terraform import (
    generate_cluster_tfvars_file,
    save_cluster_terraform_files,
)


def create_cluster():
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

        # Generate terraform files:
        info(f"Generating terraform.tfvars file for Cluster `{cluster_tag}`...")
        generate_cluster_tfvars_file(cluster_config=cluster_config)

        info(
            f"Saving cloud blueprints in a MinIO bucket for Cluster `{cluster_tag}`..."
        )
        save_cluster_terraform_files(cluster_config=cluster_config)

        # Save cluster_config to Postgres:
        instance_types = get_instance_types()
        for instance_type, vcpus in instance_types.items():
            print(f"{instance_type}: {vcpus} vCPUs")

    except KeyboardInterrupt:
        error("\nCluster creation CANCELLED by the user.")


def destroy_cluster():
    info("Destroying cluster...")
