#!/bin/bash

# Update OS packages with yum:
sudo yum -y update

# Install general system dependencies:
sudo yum groupinstall -y 'Development Tools'
sudo yum install -y \
  git \
  wget

# Clean yum cache and metadata:
sudo yum clean all \
  && sudo rm -rf /var/cache/yum

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
