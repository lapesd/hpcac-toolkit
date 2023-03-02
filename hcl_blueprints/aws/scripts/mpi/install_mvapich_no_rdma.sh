#!/bin/bash

# Install Linux kernel header files
sudo yum install -y \
    kernel-devel-$(uname -r) \
    libibverbs \
    libibverbs-devel

wget https://mvapich.cse.ohio-state.edu/download/mvapich/mv2/mvapich2-2.3.7-1.tar.gz \
    && tar -xzf mvapich2-2.3.7-1.tar.gz \
    && cd mvapich2-2.3.7-1 \
    && ./configure --enable-rdma-cd=no \
    && make -j4 \
    && sudo make install
