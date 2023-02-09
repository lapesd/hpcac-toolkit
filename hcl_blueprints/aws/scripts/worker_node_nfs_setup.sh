#!/bin/bash

# Mount the Network File System
echo "NFS Server Host: $1";
sudo mkdir -p /var/nfs_dir
sudo mount -t nfs 10.0.0.10:/var/nfs_dir /var/nfs_dir

# Add permissions
sudo chmod ugo+rwx /var/nfs_dir