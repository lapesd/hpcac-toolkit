# HPC@Cloud Toolkit

This repository contains a dockerized environment and source code for the
HPC@Cloud toolkit, comprised of a command-line interface for generating
opionated Terraform plans for Cloud HPC clusters. Currently AWS is fully
supported by the CLI, and Vultr and IBM Cloud are partially supported through
template HCL files.

# Contributing

## Setting up a development environment

Although HPC@Cloud can be executed on Windows using Docker, for developing it's
recommended to use a Linux distro or MacOS.

- Install [git](https://git-scm.com/)
- Install [docker](https://www.docker.com/)
- Install [poetry](https://python-poetry.org/) and set a virtual environment for
  Python 3.11
- Install [deno](https://deno.com/manual@v1.33.3/getting_started/installation)
