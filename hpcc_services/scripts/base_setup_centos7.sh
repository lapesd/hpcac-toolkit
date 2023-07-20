#!/bin/bash

# Scratch Centos7 AMI: ami-002070d43b0a4f171
# Base AMI (after running this script): ami-0130032104b33f31a
# Instance username: centos

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

# Mount the Elastic Block Storage:
#sudo mkfs -t ext4 /dev/sdh \
#  && sudo mkdir /var/nfs_dir \
#  && sudo mount /dev/sdh /var/nfs_dir \
#  && sudo chown nfsnobody:nfsnobody /var/nfs_dir \
#  && sudo chmod 755 /var/nfs_dir

sudo mkdir /var/nfs_dir
sudo chown nfsnobody:nfsnobody /var/nfs_dir
sudo chmod 755 /var/nfs_dir

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
