#!/bin/bash

# Install Kernel headers
sudo yum install -y kernel-devel-3.10.0-1160.76.1.el7.x86_64

# Install multilib support
sudo yum install -y glibc-devel.i686 libstdc++-devel.i686 zlib-devel.i686

# Compile BLCR from sources
git clone git@github.com:vanderlei-filho/blcr.git
cd blcr
./autogen.sh
./configure
make rpms
