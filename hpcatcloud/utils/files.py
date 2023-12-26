import os
import subprocess
import yaml


def load_yaml(yaml_file_path: str) -> dict:
    # Ensure the input YAML file exists
    if not os.path.exists(yaml_file_path):
        raise FileNotFoundError(f"{yaml_file_path} does not exist")

    # Read YAML definitions
    with open(yaml_file_path, "r") as file:
        yaml_data = yaml.safe_load(file)
    
    return yaml_data


def generate_hostfile(
    number_of_nodes: int,
    processes_per_node: int,
    hostfile_path: str
) -> None:
    base_host = "10.0.0.1"

    # Delete hostfile if existing
    if os.path.exists(hostfile_path):
        os.remove(hostfile_path)

    # Write new hostfile (OpenMPI)
    with open(hostfile_path, "w") as file:
        for i in range(number_of_nodes):
            file.write(f"{base_host}{i} slots={processes_per_node}\n")


def transfer_folder_over_ssh(
    local_folder_path: str,
    remote_destination_path: str,
    ip: str,
    user: str,
) -> None:
    # Transfer folder with scp
    subprocess.run(
        [
            "scp",
            "-o", "StrictHostKeyChecking=no",
            "-r", local_folder_path,
            f"{user}@{ip}:{remote_destination_path}",
        ],
        check=True,
    )

def delete_remote_folder_over_ssh(
    remote_folder_path: str,
    ip: str,
    user: str,
) -> None:
    # Delete the remote folder using SSH
    subprocess.run(
        [
            "ssh",
            "-o", "StrictHostKeyChecking=no",
            f"{user}@{ip}",
            f"rm -r {remote_folder_path}"
        ],
        check=True,
    )

def download_experiment_results(
    remote_folder_path: str,
    local_destination_path: str,
    ip: str,
    user: str,
) -> None:
    if remote_folder_path is None:
        # do nothing
        pass
    else:
        try:
            print("Downloading experiment result files...")

            # Check if the ./results directory exists, create it if not
            results_dir = os.path.join('.', 'results')
            if not os.path.exists(results_dir):
                os.makedirs(results_dir)

            # Download folder with scp
            subprocess.run(
                [
                    "scp",
                    "-o", "StrictHostKeyChecking=no",
                    "-r", f"{user}@{ip}:{remote_folder_path}",
                    os.path.join(results_dir, local_destination_path),
                ],
                check=True,
            )
        except Exception as e:
            print(f"Error trying to download remote folder {remote_folder_path}: {e}")
