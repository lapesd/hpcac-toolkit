#!/bin/bash

# Build and install OpenMPI with ULFM (User-Level Failure Mitigation) support from source
git clone --recursive https://github.com/open-mpi/ompi.git
cd ompi || exit
sudo ./autogen.pl --exclude oshmem
sudo ./configure --disable-io-romio CFLAGS='-O0 -g' --disable-man-pages
sudo make install && git clean -fdx && rm -rf 3rd-party
cd ..
sudo rm -r ./ompi