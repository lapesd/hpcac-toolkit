# HPC@Cloud Toolkit

This repository contains a dockerized environment and source code for the
HPC@Cloud toolkit, comprised of a command-line interface for generating
Terraform plans for Cloud HPC clusters.

# Contributing

## Setting up a development environment

Although HPC@Cloud can be executed on Windows using Docker, for developing it's
recommended to use a Linux distro or MacOS.

- Install [git](https://git-scm.com/)
- Install [docker](https://www.docker.com/)
- Install [pyenv](https://github.com/pyenv/pyenv) and set a virtual environment
  for Python 3.11
- Install [poetry](https://python-poetry.org/)
- Install [deno](https://deno.com/manual@v1.33.3/getting_started/installation) (only for frontend development)


## Running Sample Workloads


### First time setup

1. Navigate to the `./hpcc_services` directory.
2. Execute:

```shell
make docker-run-dev
```

```shell
poetry shell
```

```shell
poetry install
```

```shell
python manage.py migrate
```

### Dynemol

1. Make sure you followed the first time setup instructions.
2. Create and/or edit the `./hpcc_services/cluster_config.yaml` file with the desired cluster configuration. On AWS, make sure you set the following variables to use these AMIs:

```
master_ami: "ami-06595df5f34c718e5"
worker_ami: "ami-06595df5f34c718e5"
```

3. Create and/or edit the `./hpcc_services/cluster_init.sh` file with the following contents:

```bash
# Perform distro packages update
sudo yum update -y

# Setup Intel OneAPI environment initialization
echo 'export DYNEMOLWORKDIR=/var/nfs_dir/dynemol' >> ~/.bashrc
echo 'source /opt/intel/oneapi/setvars.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc
```

3. Run ```make create-cluster``` to quickly create a test cluster, or:

```shell
python manage.py create_cluster_config cluster_config.yaml
```

Followed by:

```shell
python manage.py create_cluster ${cluster_label set at cluster_config.yaml}
```

4. Edit the `./sample_workloads/dynemol/hostfile` file according to your created cluster.

5. Copy dynemol input files to the cluster using `scp`:

```shell
scp -r ../sample_workloads/dynemol ec2-user@${master_node_public_ip_address}:/var/nfs_dir
```

6. Run mpiexec to execute Dynemol, setting the appropriate number of processes:

```shell
ssh ec2-user@${master_node_public_ip_address} "cd /var/nfs_dir/dynemol && mpiexec --hostfile /var/nfs_dir/dynemol/hostfile -n 16 --map-by ppr:4:node:PE=4 --bind-to core /home/ec2-user/Dynemol/dynemol"
```

7. Don't forget to destroy the created resources after your tests:

```shell
make destroy-cluster
```

or 

```shell
python manage.py destroy_cluster
```
