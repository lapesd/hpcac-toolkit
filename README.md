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
- Install [terraform](https://developer.hashicorp.com/terraform/install?product_intent=terraform)
- Install [aws cli](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html), if you plan to use AWS
- Install [pyenv](https://github.com/pyenv/pyenv#installation) and set a virtual environment
  for Python 3.11 (you'll need a bunch of other dependencies to successfully use pyenv, refer to it's own documentation or install any package required if you get errors during pyenv's installation)
- Install [poetry](https://python-poetry.org/)

### First time setup

1. Run Docker and launch the dev containers:

```shell
make init
```

2. Launch a Python environment with Poetry:

```shell
poetry shell
```

3. You then can install the dependencies with the command below. You may need to install `libpq-dev` or other packages to be able to compile `psycopg2`:

```shell
poetry install
```

3. Run Django migrations:

```shell
python manage.py migrate
```

4. If you plan to use AWS, generate credentials (ACCESS and SECRET keys) and run the following to create a default profile:

```shell
aws configure
```

It should create the file `~/.aws/configure` with the `default` profile.

You are now ready to use HPC@Cloud!

### The `cluster_init.sh` file

Copy the `cluster_init.example.sh` file and rename it to `cluster_init.sh`. Edit
this file as you like (it will be executed in every cluster node after spawn).

### The `cluster_config.yaml` file

To create a cluster configuration, copy the `cluster_config.example.yaml` file
and rename it to `cluster_config.yaml`. Edit the file as you like, and then run
from the command-line:

```shell
make create-cluster
```

After the execution completes, the cluster will be available over SSH
connection.

To destroy your cluster, run:

```shell
make destroy-cluster
```

### The `mpi_run.yaml` file and the `my_files` directory

Place all the files you want to transfer to the cluster inside the `my_files` folder.
To setup a MPI workload to be executed, copy the `mpi_run.example.yaml` file
and rename it to `mpi_run.yaml`. Edit the file as you like, and then run
from the command-line:

```shell
make mpi-launch
```

# Publications

Please, find bellow the list of publications related to the HPC@Cloud Toolkit. If you need to cite HPC@Cloud Toolkit, please reference [*HPC@Cloud: A Provider-Agnostic Software Framework for Enabling HPC in Public Cloud Platforms*](https://doi.org/10.5753/wscad.2022.226528) for a general presentation.

- Vanderlei Munhoz, Márcio Castro, Odorico Mendizabal. *Strategies for Fault-Tolerant Tightly-coupled HPC Workloads Running on Low-Budget Spot Cloud Infrastructures*. **International Symposium on Computer Architecture and High Performance Computing (SBAC-PAD)**. Bordeaux, France: IEEE Computer Society, 2022. [[link]](https://doi.org/10.1109/SBAC-PAD55451.2022.00037) [[bib]](http://www.inf.ufsc.br/~marcio.castro/bibs/2022_sbacpad.bib)

- Vanderlei Munhoz, Márcio Castro. *HPC@Cloud: A Provider-Agnostic Software Framework for Enabling HPC in Public Cloud Platforms*. **Simpósio em Sistemas Computacionais de Alto Desempenho (WSCAD)**. Florianópolis, Brazil: SBC, 2022. [[link]](https://doi.org/10.5753/wscad.2022.226528) [[bib]](http://www.inf.ufsc.br/~marcio.castro/bibs/2022_wscad.bib)

- Daniel Cordeiro, Emilio Francesquini, Marcos Amaris, Márcio Castro, Alexandro Baldassin, João Vicente Lima. *Green Cloud Computing: Challenges and Opportunities*. **Simpósio Brasileiro de Sistemas de Informação (SBSI)**. Maceió, Brazil: SBC, 2023. [[link]](http://dx.doi.org/10.5753/sbsi_estendido.2023.229291)

- Livia Ferrão, Vanderlei Munhoz, Márcio Castro. *Análise do Sobrecusto de Utilização de Contêineres para Execução de Aplicações de HPC na Nuvem*. **Escola Regional de Alto Desempenho da Região Sul (ERAD/RS)**. Porto Alegre, Brazil: SBC, 2023. [[link]](http://dx.doi.org/10.5753/eradrs.2023.229787)

- Luiz Fernando Althoff, Vanderlei Munhoz, Márcio Castro. *Análise de Viabilidade do Perfilamento de Aplicações de HPC Baseada em Contadores de Hardware na AWS*. **Escola Regional de Alto Desempenho da Região Sul (ERAD/RS)**. Porto Alegre, Brazil: SBC, 2023. [[link]](http://dx.doi.org/10.5753/eradrs.2023.230088)
