#!/bin/bash

# Update OS packages with yum:
sudo yum -y update

# Install general system dependencies:
sudo yum groupinstall -y 'Development Tools'
sudo yum install -y \
  git \
  wget \
  nfs-utils

# Clean yum cache and metadata:
sudo yum clean all \
  && sudo rm -rf /var/cache/yum

# Setup EFS/NFS shared directories
sudo mkdir /var/nfs_dir
sudo chown -R nobody:nobody /var/nfs_dir
sudo chown -R ec2-user:nobody /var/nfs_dir
sudo chmod -R 775 /var/nfs_dir

# Edit the /etc/ssh/ssh_config file with the following:
# Host *
#    StrictHostKeyChecking no
#    UserKnownHostsFile=/dev/null
sudo rm /etc/ssh/ssh_config
sudo touch /etc/ssh/ssh_config
sudo sh -c "echo 'Host *' >> /etc/ssh/ssh_config"
sudo sh -c "echo '        StrictHostKeyChecking no' >> /etc/ssh/ssh_config"
sudo sh -c "echo '        UserKnownHostsFile=/dev/null' >> /etc/ssh/ssh_config"

# Edit the /root/.ssh/authorized_keys to allow root ssh access:
sudo rm /root/.ssh/authorized_keys
sudo touch /root/.ssh/authorized_keys
sudo sh -c "echo '10.0.0.10' >> /root/.ssh/authorized_keys"
