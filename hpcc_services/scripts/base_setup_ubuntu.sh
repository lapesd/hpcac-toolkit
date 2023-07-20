#!/bin/bash

# Base AMI: 
# - ami-0261755bbcb8c4a84 (20 LTS)
# - ami-0dafae4ace57cf0bc (custom based on 20 LTS)
# - ami-0c5e5f9f065ecab06 (18 LTS)
# Instance username: ubuntu

# Update OS packages with yum:
sudo apt-get update -y
sudo apt-get upgrade -y

# Install general system dependencies:
sudo apt-get install -y build-essential
sudo apt-get install -y \
  git \
  wget \
  nfs-common

# Clean yum cache and metadata:
sudo apt-get clean
sudo apt-get autoclean

# Mount the Elastic Block Storage:
#sudo mkfs -t ext4 /dev/sdh \
#  && sudo mkdir /var/nfs_dir \
#  && sudo mount /dev/sdh /var/nfs_dir \
#  && sudo chown nobody:nogroup /var/nfs_dir \
#  && sudo chmod 755 /var/nfs_dir

sudo mkdir /var/nfs_dir
sudo chown nobody:nogroup /var/nfs_dir
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
