import os

from hpcac_cli.utils.minio import create_minio_bucket, upload_file_to_minio_bucket


def generate_cluster_tfvars_file(cluster_config: dict) -> str:
    # Create temporary directory for HCL files:
    os.makedirs(os.path.dirname("./tmp_terraform_dir/"), exist_ok=True)

    # Generate terraform.tfvars file:
    with open("./tmp_terraform_dir/terraform.tfvars", "w") as output_tfvars_file:
        for key, value in cluster_config.items():
            if key not in [
                "provider",
                "cluster_tag",
            ]:  # provider and tag aren't tfvars
                if isinstance(value, (int, float)) or (
                    isinstance(value, str) and value.isdigit()
                ):
                    output_tfvars_file.write(f"{key} = {value}\n")
                else:
                    output_tfvars_file.write(f'{key} = "{value}"\n')


def save_cluster_terraform_files(cluster_config: dict):
    # Create MinIO bucket for cluster files:
    cluster_bucket_name = cluster_config["tag"].replace(" ", "").replace("_", "-")
    create_minio_bucket(cluster_bucket_name)

    # Upload Cluster terraform.tfvars file to MinIO bucket:
    upload_file_to_minio_bucket(
        file_name_local="./tmp_terraform_dir/terraform.tfvars",
        file_name_in_bucket="terraform.tfvars",
        bucket_name=cluster_bucket_name,
    )

    # Copy cloud blueprints to MinIO bucket based on the selected provider:
    for file_name in ["versions.tf", "provider.tf", "cluster.tf"]:
        upload_file_to_minio_bucket(
            file_name_local=f"./cloud_blueprints/{cluster_config['provider']}/{file_name}",
            file_name_in_bucket=file_name,
            bucket_name=cluster_bucket_name,
        )
