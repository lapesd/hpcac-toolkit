### Dynemol

1. Make sure you followed the first time setup instructions.
2. Create and/or edit the `./cluster_config.yaml` file with the desired cluster
   configuration. On AWS, make sure you set the following variables to use these
   AMIs:

```
master_ami: "ami-06595df5f34c718e5"
worker_ami: "ami-06595df5f34c718e5"
```

3. Create and/or edit the `./cluster_init.sh` file with the following contents:

```bash
# Perform distro packages update
sudo yum update -y

# Setup Intel OneAPI environment initialization
echo 'export DYNEMOLWORKDIR=/var/nfs_dir/dynemol' >> ~/.bashrc
echo 'source /opt/intel/oneapi/setvars.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc
```

3. Run `make create-cluster` to quickly create a test cluster, or:

```shell
python manage.py create_cluster_config cluster_config.yaml
```

Followed by:

```shell
python manage.py create_cluster ${cluster_label set at cluster_config.yaml}
```

4. Edit the `./sample_workloads/dynemol/hostfile` file according to your created
   cluster.

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
