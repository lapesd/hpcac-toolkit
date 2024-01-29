import os
import subprocess

from hpcac_cli.utils.logger import Logger
from hpcac_cli.utils.minio import (
    create_minio_bucket,
    upload_file_to_minio_bucket,
    download_file_from_minio_bucket,
)


log = Logger()
TF_DIR = "./tmp_terraform_dir"
TERRAFORM_FILES = ["versions.tf", "provider.tf", "cluster.tf"]


def generate_cluster_tfvars_file(cluster_config: dict) -> str:
    # Create temporary directory for HCL files:
    os.makedirs(os.path.dirname(TF_DIR), exist_ok=True)

    # Generate terraform.tfvars file:
    with open(f"{TF_DIR}/terraform.tfvars", "w") as output_tfvars_file:
        for key, value in cluster_config.items():
            if key not in ["provider", "init_commands"]:  # provider is not a tfvar
                if isinstance(value, bool):
                    output_tfvars_file.write(
                        f'{key} = {"true" if value else "false"}\n'
                    )
                elif isinstance(value, (int, float)) or (
                    isinstance(value, str) and value.isdigit()
                ):
                    output_tfvars_file.write(f"{key} = {value}\n")
                else:
                    output_tfvars_file.write(f'{key} = "{value}"\n')


def save_cluster_terraform_files(cluster_config: dict):
    # Create MinIO bucket for cluster files:
    cluster_bucket_name = (
        cluster_config["cluster_tag"].replace(" ", "").replace("_", "-")
    )
    create_minio_bucket(cluster_bucket_name)

    # Copy cloud blueprints to MinIO bucket based on the selected provider:
    for file_name in TERRAFORM_FILES:
        upload_file_to_minio_bucket(
            file_path=f"./cloud_blueprints/{cluster_config['provider']}/{file_name}",
            object_name=file_name,
            bucket_name=cluster_bucket_name,
        )


def get_cluster_terraform_files(cluster_config: dict):
    cluster_bucket_name = (
        cluster_config["cluster_tag"].replace(" ", "").replace("_", "-")
    )

    for file_name in TERRAFORM_FILES:
        download_file_from_minio_bucket(
            bucket_name=cluster_bucket_name,
            object_name=file_name,
            file_path=os.path.abspath(f"{TF_DIR}/{file_name}"),
        )


def launch_subprocess(commands: list[str], detail: str, verbose: bool = False) -> int:
    process = subprocess.Popen(
        commands,
        cwd=TF_DIR,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )

    log.debug(text="Please wait...", detail=detail)
    for line in iter(process.stdout.readline, ""):
        if verbose:
            log.debug(text=line.strip(), detail=detail)
    process.stdout.close()
    return_code = process.wait()

    return return_code


def terraform_init(verbose: bool = False):
    launch_subprocess(
        commands=["terraform", "init"], detail="terraform init", verbose=verbose
    )


def terraform_refresh(verbose: bool = False):
    launch_subprocess(
        commands=["terraform", "refresh"], detail="terraform refresh", verbose=verbose
    )
    launch_subprocess(
        commands=["terraform", "plan"], detail="terraform plan", verbose=verbose
    )
    launch_subprocess(
        commands=["terraform", "apply", "-auto-approve"],
        detail="terraform apply",
        verbose=verbose,
    )


def terraform_apply(verbose: bool = False, retry: bool = False):
    max_retries = 3  # Define the maximum number of retries
    retry_count = 0

    while True:
        exit_code = launch_subprocess(
            commands=["terraform", "apply", "-auto-approve"],
            detail="terraform apply",
            verbose=verbose,
        )

        if exit_code == 0:
            break  # Break if successful

        if retry and retry_count < max_retries:
            retry_count += 1
            log.debug(
                text=f"Retrying terraform apply... (Attempt {retry_count}/{max_retries})",
                detail="terraform apply",
            )
        else:
            log.error(text="Terraform apply failed.", detail="terraform apply")
            break


def terraform_destroy(verbose: bool = False):
    launch_subprocess(
        commands=["terraform", "destroy", "-auto-approve"],
        detail="terraform destroy",
        verbose=verbose,
    )
