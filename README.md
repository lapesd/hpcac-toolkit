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

### First time setup

1. Run Docker and launch the dev containers:

```shell
make docker-run-dev
```

2. Launch and setup the Python environment with Poetry:

```shell
poetry shell
```

```shell
poetry install
```

3. Run Django migrations:

```shell
python manage.py migrate
```

You are now ready to use HPC@Cloud!

### The `cluster_init.sh` file

Copy the `cluster_init.example.sh` file and rename it to `cluster_init.sh`. Edit
this file as you like (it will be executed in every cluster node after spawn).

### The `cluster_config.yaml` file

To create a cluster configuration, copy the `cluster_config.example.yaml` file
and rename it to `cluster_config.yaml`. Edit the file as you like, and then run
from the command-line:

```shell
python manage.py create_cluster_config cluster_config.yaml
```

Then, to create a cluster from the created cluster configuration, run from the
command-line:

```shell
python manage.py create_cluster <cluster_label>
```

The `cluster_label` is the same variable set at the `cluster_config.yaml` file.

After the execution completes, the cluster will be available over SSH
connection.
