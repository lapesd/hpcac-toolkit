import os
import sys
import yaml

from django.core.management.base import BaseCommand
from minio import Minio

from hpcc_api.clusters.models import ClusterConfiguration
from hpcc_api.exceptions import ConfigurationError


def generate_cluster_blueprint_from_yaml_definitions(
    yaml_file_path: str,
) -> ClusterConfiguration:
    # Ensure the input YAML file exists
    if not os.path.exists(yaml_file_path):
        raise FileNotFoundError(f"{yaml_file_path} does not exist")

    # Read YAML definitions
    with open(yaml_file_path, "r") as file:
        yaml_data = yaml.safe_load(file)
        provider = yaml_data.get("provider")
        if provider is None:
            raise ConfigurationError(
                f"Missing required `provider` key in {yaml_file_path}"
            )
        elif provider not in ClusterConfiguration.DEFAULTS:
            raise ConfigurationError(f"Provider `{provider}` not supported.")

        cluster_label = yaml_data.get("cluster_label")
        if cluster_label is None:
            raise ConfigurationError(
                f"Missing required `cluster_label` key in {yaml_file_path}"
            )

    # Merge default and input YAML definitions
    cluster_options = {**ClusterConfiguration.DEFAULTS[provider], **yaml_data}
    use_spot = cluster_options.get("use_spot")

    # Generate the terraform.tfvars file
    os.makedirs(os.path.dirname("./tmp_terraform_dir/"), exist_ok=True)
    with open("./tmp_terraform_dir/terraform.tfvars", "w") as output_tfvars_file:
        for key, value in cluster_options.items():
            if key not in ["provider", "cluster_label"]:
                if isinstance(value, (int, float)) or (
                    isinstance(value, str) and value.isdigit()
                ):
                    output_tfvars_file.write(f"{key} = {value}\n")
                else:
                    output_tfvars_file.write(f'{key} = "{value}"\n')

    # Create a MinIO bucket for this ClusterConfiguration
    minio = Minio(
        "localhost:9000",
        access_key="root",
        secret_key="password",
        secure=False,
    )
    minio_bucket_name = f"{cluster_label.replace('_', '-')}-bucket"
    if not minio.bucket_exists(minio_bucket_name):
        minio.make_bucket(minio_bucket_name)

    # Upload the generated terraform.tfvars to the ClusterConfiguration's MinIO bucket
    minio.fput_object(
        minio_bucket_name,
        "terraform.tfvars",
        os.path.abspath(f"./tmp_terraform_dir/terraform.tfvars"),
    )
    print("Saved terraform.tfvars to MinIO.")

    # Upload the Cluster blueprints to the ClusterConfiguration's MinIO bucket
    for file_name in ["versions.tf", "provider.tf", "cluster.tf"]:
        minio_response = minio.fput_object(
            minio_bucket_name,
            file_name,
            os.path.abspath(f"./hcl_blueprints/{provider}/{file_name}"),
        )
        print(
            "Created `{0}` object with etag: `{1}` at bucket `{2}`".format(
                minio_response.object_name,
                minio_response.etag,
                minio_bucket_name,
            )
        )

    # Create and save the ClusterConfiguration object
    cluster_config, _created = ClusterConfiguration.objects.update_or_create(
        label=cluster_options.get("cluster_label"),
        defaults={
            "cloud_provider": provider,
            "nodes": cluster_options.get("worker_count") + 1,
            "transient": True if use_spot.lower() == "true" else False,
            "minio_bucket_name": minio_bucket_name,
        },
    )

    return cluster_config


class Command(BaseCommand):
    help = "Creates a ClusterConfiguration from a YAML configuration file."

    def add_arguments(self, parser):
        parser.add_argument(
            "yaml_file",
            type=str,
            help="The source YAML file",
        )
        parser.add_argument(
            "--singularity",
            action="store_true",
            help="Set this flag to activate Singularity support",
        )

    def handle(self, *args, **options):
        yaml_file = options["yaml_file"]

        try:
            cluster_config = generate_cluster_blueprint_from_yaml_definitions(yaml_file)

        except Exception as error:
            self.stdout.write(self.style.ERROR(f"CommandError: {error}"))
            sys.exit(1)

        self.stdout.write(
            self.style.SUCCESS(
                f"Successfully created the `{cluster_config.label}` cluster blueprint."
            )
        )
