#!/bin/bash

# Run this only if you need manpages
# Install Python and Sphinx (if not already installed)
#sudo yum install python3 python3-pip
#sudo pip3 install Sphinx sphinx_rtd_theme recommonmark

# Clone the Open MPI repository
git clone --recursive https://github.com/open-mpi/ompi.git
cd ompi

# Generate the configuration scripts
sudo ./autogen.pl

# Run this only if you want manpages and debug
# Configure Open MPI with debug flags and enable man pages (for Sphinx)
#sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g' --enable-man-pages

sudo ./configure --disable-io-romio CFLAGS='-O3'

# Build and install
#sudo make all install
sudo make -j$(nproc) all install

# Clean up
sudo git clean -fdx
sudo rm -rf 3rd-party
