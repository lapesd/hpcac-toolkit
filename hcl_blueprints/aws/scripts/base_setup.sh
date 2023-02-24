#!/bin/bash

# Apply OS packages updates
sudo yum -y update

# Install system dependencies
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
  make \
  libtool \
  flex \
  openssl-devel \
  glibc-devel \
  patch \
  libuuid-devel \
  libseccomp-devel \
  pkg-config \
  cryptsetup \
  libibverbs-core \
  libibverbs-devel \
  libfabric-devel \
  openmpi-devel

# Install GO Language support (required for SingularityCE)
wget https://dl.google.com/go/go1.13.linux-amd64.tar.gz
sudo tar --directory=/usr/local -xzvf go1.13.linux-amd64.tar.gz
export PATH=/usr/local/go/bin:$PATH

# Compile SingularityCE from sources
wget https://github.com/singularityware/singularity/releases/download/v3.5.3/singularity-3.5.3.tar.gz
tar -xzvf singularity-3.5.3.tar.gz
cd singularity
./mconfig
cd builddir
make
sudo make install
cd ..
cd ..
rm singularity-3.5.3.tar.gz
rm go1.13.linux-amd64.tar.gz

# make file hello_world.c 
cat <<EOF >hello_world.c

#include <mpi.h>
#include <stdio.h>

int main(int argc, char** argv) {
    // Initialize the MPI environment
    MPI_Init(NULL, NULL);

    // Get the number of processes
    int world_size;
    MPI_Comm_size(MPI_COMM_WORLD, &world_size);

    // Get the rank of the process
    int world_rank;
    MPI_Comm_rank(MPI_COMM_WORLD, &world_rank);

    // Get the name of the processor
    char processor_name[MPI_MAX_PROCESSOR_NAME];
    int name_len;
    MPI_Get_processor_name(processor_name, &name_len);

    // Print off a hello world message
    printf("Hello world from processor %s, rank %d out of %d processors\n",
           processor_name, world_rank, world_size);

    // Finalize the MPI environment.
    MPI_Finalize();
}
EOF

# Install MVAPICH with AWS support from RPM package release
wget http://mvapich.cse.ohio-state.edu/download/mvapich/mv2x/2.3/mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm
sudo rpm -Uvh --nodeps mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm
rm mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm

# # Build and install OpenMPI with ULFM (User-Level Failure Mitigation) support from source
# git clone --recursive https://github.com/open-mpi/ompi.git
# cd ompi || exit
# sudo ./autogen.pl --no-oshmem
# sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g' --disable-man-pages
# sudo make install && git clean -fdx && rm -rf 3rd-party

singularity pull --arch amd64 library://livia_ferrao/image/mvapich4:latest

# INSTALL NAS BENCHMARK
wget https://www.nas.nasa.gov/assets/npb/NPB3.3.1.tar.gz
tar -zxvf NPB3.3.1.tar.gz
rm NPB3.3.1.tar.gz 
cd NPB3.3.1/NPB3.3-MPI/config
cat <<EOF > make.def
	MPICC = cc
	CLINK   = cc
	CMPI_LIB  = -L/opt/mvapich2-x/gnu7.3.1/mofed/aws/mpirun/lib64 -lmpi
	CMPI_INC = -I/opt/mvapich2-x/gnu7.3.1/mofed/aws/mpirun/include
	CFLAGS  = -O
	CLINKFLAGS = -O
	CC  = cc -g
	BINDIR  = ../bin
	RAND   = randi8
EOF
cd ..
make IS CLASS=A NPROCS=1
make IS CLASS=A NPROCS=2
make DT CLASS=A NPROCS=1

# EXPORTS
cat <<EOF > ~/.bash_profile
# .bash_profile
# Get the aliases and functions
if [ -f ~/.bashrc ]; then
	. ~/.bashrc
fi
# User specific environment and startup programs
PATH=$PATH:$HOME/.local/bin:$HOME/bin:/opt/mvapich2-x/gnu7.3.1/mofed/aws/mpirun/bin
LD_LIBRARY_PATH=/opt/mvapich2-x/gnu7.3.1/mofed/aws/mpirun/lib64:$LD_LIBRARY_PATH
export MV2_SMP_USE_CMA=0
export MV2_ENABLE_AFFINITY=0 
export PATH
export LD_LIBRARY_PATH
EOF

source ~/.bash_profile

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
