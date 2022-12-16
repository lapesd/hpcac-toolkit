#!/bin/bash

# Setup the Network File System
sudo systemctl unmask rpcbind && sudo systemctl enable rpcbind && sudo systemctl start rpcbind
sudo systemctl enable nfs-server && sudo systemctl start nfs-server
sudo systemctl enable nfs-lock && sudo systemctl start nfs-lock
sudo systemctl enable nfs-idmap && sudo systemctl start nfs-idmap
sudo systemctl enable rpc-statd && sudo systemctl start rpc-statd
sudo sh -c "echo '/var/nfs_dir      *(rw,sync,no_root_squash)' >> /etc/exports"
sudo systemctl restart nfs-server

# Add permissions
sudo chmod ugo+rwx /var/nfs_dir