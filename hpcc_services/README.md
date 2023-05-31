# HPCC_SERVICES

This is the HPC@Cloud command-line application, based on Django.

## How to use

To create a cluster configuration, copy the `cluster_config.example.yaml` file
and rename it to `cluster_config.yaml`. Edit the file as you like, and then run
from the command-line:

```shell
python manage.py create_cluster_config cluster_config.yaml
```

Then, to create a cluster from the created cluster configuration, run from the
command-line (The `cluster_label` is the same variable set at the
`cluster_config.yaml` file):

```shell
python manage.py create_cluster <cluster_label>
```

After the execution completes, the cluster will be available over SSH
connection.
