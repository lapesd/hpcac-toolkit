# HPC@Cloud V1.0.0

This version was discontinued in favor of a self-contained Rust-based CLI.
Please refer to the [main branch](https://github.com/lapesd/hpcac-toolkit) for more information.
If you are still interested in the V1.0.0 version of HPC@Cloud, you can find the instructions to run it here.

### Setting up a development environment

Although HPC@Cloud can be executed on Windows using Docker, for developing it's recommended to use a Linux distro or MacOS.

To get started:

1. **Install prerequisites:**
	- Install [git](https://git-scm.com/)
	- Install [docker](https://www.docker.com/)
	- Install [terraform](https://developer.hashicorp.com/terraform/install?product_intent=terraform)
	- Install [aws cli](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html), if you plan to use AWS
	- Install [python](https://www.python.org/downloads/) version 3.11 or higher (ideally, install [pyenv](https://github.com/pyenv/pyenv#installation) and setup a virtual environment for HPC@Cloud)


2. **First time setup**
	- Run Docker and launch the dev containers with the following:
	```shell
	make init
	```

	- You then can install HPC@Cloud and its dependencies with the command below. You may need to install `libpq-dev` or other packages to be able to compile `psycopg2`:
	```shell
	make install
	```

	- If you plan to use AWS, generate credentials (ACCESS and SECRET keys)
		- To generate your AWS Access and Secret keys, follow the instructions in the official AWS documentation: 
			- [Managing Access Keys for IAM Users](https://docs.aws.amazon.com/IAM/latest/UserGuide/access-key-self-managed.html) 
			- [Managing Access Keys for the Root User](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_root-user_manage_add-key.html)
	- And run the following to create a default profile:
	```shell
	aws configure
	```
	It should create the file `~/.aws/configure` with the `default` profile.

You are now ready to use HPC@Cloud!

---

### The `cluster_config.yaml` Setup 

1. To create a cluster configuration, copy the `cluster_config.example.yaml` file and rename it to `cluster_config.yaml`. 
	- **Note that the example file is just a template and may not work correctly in all environments.** The settings provided in the example file (e.g., AWS region, instance types, SSH keys, and storage configurations) should be adjusted according to your own infrastructure and requirements.


2. Once you've edited the file to match your requirements, run the following from the command-line:
```shell
hpcac create-cluster
```
After the execution completes, the cluster will be available over SSH connection.


3. To destroy your cluster, run:
```shell
hpcac destroy-cluster
```

---

### Setting up Tasks with `tasks_config.yaml` File and the `my_files` Directory

1. Place all the files you want to transfer to the cluster inside the `my_files` folder.

2. To set up a task to be executed in the cluster, copy the `tasks_config.example.yaml` file and rename it to `tasks_config.yaml`. 
	- **Note that the example file is just a template.** You should edit this file to match the specific requirements of your application, such as the commands to be run, file paths, and any other necessary configurations.

3. Once you've edited the file to fit your needs, run the following from the command-line:
```shell
hpcac run-tasks
```
**Note: The results produced by the tasks should be saved in the './result' folder.**
