#!/bin/bash

# Apply OS packages updates
sudo yum -y update

# Install dependencies
sudo yum install -y \
  wget \
  perl \
  gcc \
  gcc-c++ \
  gcc-gfortran \
  nfs-utils \
  git \
  autoconf \
  automake \
  m4 \
  libtool \
  flex \
  openssl-devel

# Clean yum cache and metadata
sudo yum clean all
sudo rm -rf /var/cache/yum

# Disable FirewallD and install Iptables
sudo yum remove -y firewalld \
&& sudo yum install -y iptables-services \
&& sudo systemctl start iptables \
&& sudo systemctl enable iptables \
&& sudo iptables -L -n \
&& sudo /usr/libexec/iptables/iptables.init save

# Setup Iptables to allow all traffic
iptables -F \
&& iptables -X \
&& iptables -t nat -F \
&& iptables -t nat -X \
&& iptables -t mangle -F \
&& iptables -t mangle -X \
&& iptables -P INPUT ACCEPT \
&& iptables -P FORWARD ACCEPT \
&& iptables -P OUTPUT ACCEPT

# Edit the /etc/ssh/ssh_config file with the following:
# Host *
#    StrictHostKeyChecking no
#    UserKnownHostsFile=/dev/null
sudo rm /etc/ssh/ssh_config
sudo touch /etc/ssh/ssh_config
sudo sh -c "echo 'Host *' >> /etc/ssh/ssh_config"
sudo sh -c "echo '        StrictHostKeyChecking no' >> /etc/ssh/ssh_config"
sudo sh -c "echo '        UserKnownHostsFile=/dev/null' >> /etc/ssh/ssh_config"

# Edit the /root/.ssh/authorized_keys to allow root ssh access
sudo rm /root/.ssh/authorized_keys
sudo touch /root/.ssh/authorized_keys
sudo sh -c "echo '$1' >> /root/.ssh/authorized_keys"
