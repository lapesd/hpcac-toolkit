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

# Build and install OpenMPI with ULFM (User-Level Failure Mitigation) support from source
git clone --recursive https://github.com/open-mpi/ompi.git
cd ompi || exit
sudo ./autogen.pl --no-oshmem
sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g' --disable-man-pages
sudo make install && git clean -fdx && rm -rf 3rd-party

# Mount the Elastic Block Storage
sudo mkfs -t ext4 /dev/xvdh
sudo mkdir /var/nfs_dir
sudo mount /dev/xvdh /var/nfs_dir
sudo chown nfsnobody:nfsnobody /var/nfs_dir
sudo chmod 755 /var/nfs_dir

# Clean yum cache and metadata
sudo yum clean all
sudo rm -rf /var/cache/yum

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