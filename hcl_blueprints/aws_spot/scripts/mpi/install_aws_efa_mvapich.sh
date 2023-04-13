#!/bin/bash

# TODO: install EFA support packages
# https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/efa-start.html#efa-start-security

# Install Linux kernel header files
sudo yum install -y kernel-devel-$(uname -r)

# Install MVAPICH with AWS support from RPM package release
# https://mvapich.cse.ohio-state.edu/userguide/mv2x-aws/
wget http://mvapich.cse.ohio-state.edu/download/mvapich/mv2x/2.3/mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm
sudo rpm -Uvh --nodeps mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm

# Cleanup
rm mvapich2-x-aws-mofed-gnu7.3.1-2.3x-1.amzn2.x86_64.rpm

# Update PATH
echo 'export PATH=/opt/mvapich2-x/gnu7.3.1/mofed/aws/mpirun/bin:${PATH}' >> ~/.bashrc && \
    source ~/.bashrc

# Prepend mvapich2-x library to LD_LIBRARY_PATH
echo 'export LD_LIBRARY_PATH=/opt/mvapich2-x/gnu7.3.1/aws-ofed/intermediate/mpirun/lib64/:${LD_LIBRARY_PATH}' >> ~/.bashrc && \
    source ~/.bashrc

# Install XPMEM for better intra-node performance
git clone https://github.com/lapesd/xpmem.git
cd xpmem && \
    ./autogen.sh && \
    ./configure --prefix=/opt/xpmem && \
    sudo make -j8 install && \
    cd .. && \
    sudo rm -r xpmem

# Load XPMEM
# You can check if XPMEM is loaded with this command: `lsmod | grep xpmem`
sudo insmod /opt/xpmem/lib/modules/5.10.82-83.359.amzn2.x86_64/kernel/xpmem/xpmem.ko && \
    sudo chmod 666 /dev/xpmem
