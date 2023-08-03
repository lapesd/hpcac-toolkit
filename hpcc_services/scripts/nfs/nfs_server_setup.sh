#!/bin/bash

# Setup the Network File System
sudo systemctl unmask rpcbind && sudo systemctl enable rpcbind && sudo systemctl start rpcbind
sudo systemctl enable nfs-server && sudo systemctl start nfs-server
sudo sh -c "echo '/var/nfs_dir      *(rw,sync,no_root_squash)' >> /etc/exports"
sudo systemctl restart nfs-server

# Add permissions
sudo chmod ugo+rwx /var/nfs_dir
