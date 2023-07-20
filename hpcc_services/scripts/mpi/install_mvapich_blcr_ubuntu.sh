#!/bin/bash

# Install Linux kernel header files
sudo apt-get update
sudo apt-get install -y \
    linux-headers-$(uname -r) \
    libibverbs1 \
    libibverbs-dev \
    gfortran \
    byacc \
    libcr-dev \
    blcr-util

# Clean apt cache
sudo apt-get clean
sudo apt-get autoclean
sudo apt-get autoremove --purge

wget https://mvapich.cse.ohio-state.edu/download/mvapich/mv2/mvapich2-2.3.7-1.tar.gz \
    && tar -xzf mvapich2-2.3.7-1.tar.gz \
    && cd mvapich2-2.3.7-1 \
    && ./configure --with-device=ch3:sock --disable-shared --enable-ckpt \
    && make -j4 \
    && sudo make install

echo 'export CR_CHKPT_DIR=/var/nfs_dir' >> ~/.bashrc
