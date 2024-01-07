import os
import yaml


def parse_yaml(config_file: str):
    if not os.path.isfile(config_file):
        raise FileNotFoundError(
            f"'{config_file}' not found in the current directory. "
            "Use the `cluster_config.example.yaml` file to create yours."
        )

    # Load the YAML configuration file into a Python dictionary
    with open(config_file, "r") as f:
        config_data = yaml.safe_load(f)

    return config_data
