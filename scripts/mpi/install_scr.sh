#!/bin/bash

SCR_ROOT="$HOME/scr"

echo "export SCR_ROOT=\"$SCR_ROOT\"" >> ~/.bashrc
source ~/.bashrc

# Build and install SCR (Scalable Checkpoint / Restart)
wget https://github.com/LLNL/scr/releases/download/v3.0/scr-v3.0.tgz
tar -zxf scr-v3.0.tgz
cd scr-v3.0

mkdir build
cd build
cmake -DSCR_RESOURCE_MANAGER=NONE -DENABLE_TESTS=OFF -DENABLE_FORTRAN=OFF -DENABLE_PDSH=OFF -DENABLE_EXAMPLES=OFF -DCMAKE_INSTALL_PREFIX=$SCR_ROOT ..
make -j install

cd ../..
rm -rf scr-v3.0*