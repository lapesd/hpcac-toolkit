import os
import yaml


def parse_yaml(config_file: str):
    if not os.path.isfile(config_file):
        raise FileNotFoundError(config_file)

    # Load the YAML configuration file into a Python dictionary
    with open(config_file, "r") as f:
        config_data = yaml.safe_load(f)

    # TODO: add basic input validation

    return config_data
