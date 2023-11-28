#!/bin/bash

# Need package "flex" to be installed without errors

# Build and install OpenMPI with ULFM (User-Level Failure Mitigation) support from source
git clone --recursive https://github.com/open-mpi/ompi.git
cd ompi || exit
sudo ./autogen.pl
sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g' --disable-man-pages
sudo make install
sudo git clean -fdx
sudo rm -rf 3rd-party
