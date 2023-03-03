#!/bin/bash

# Install SingularityCE dependencies
sudo yum install -y \
    libseccomp-devel \
    glib2-devel \
    squashfs-tools \
    cryptsetup \
    runc

# Clean yum cache and metadata:
sudo yum clean all \
  && sudo rm -rf /var/cache/yum

# Install GO Language support (required for SingularityCE)
export VERSION=1.20.1 OS=linux ARCH=amd64 && \
    wget https://dl.google.com/go/go$VERSION.$OS-$ARCH.tar.gz && \
    sudo tar -C /usr/local -xzvf go$VERSION.$OS-$ARCH.tar.gz && \
    rm go$VERSION.$OS-$ARCH.tar.gz

echo 'export GOPATH=${HOME}/go' >> ~/.bashrc && \
    echo 'export PATH=/usr/local/go/bin:${PATH}:${GOPATH}/bin' >> ~/.bashrc && \
    source ~/.bashrc

# Build Singularity from source
git clone --recurse-submodules https://github.com/sylabs/singularity.git && \
    cd singularity && \
    git checkout --recurse-submodules v3.11.0 && \
    ./mconfig && \
    make -C builddir && \
    sudo make -C builddir install
