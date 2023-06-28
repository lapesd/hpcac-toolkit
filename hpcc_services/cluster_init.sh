# Perform distro packages update
sudo yum update -y

# Setup Intel OneAPI environment initialization
echo 'export DYNEMOLWORKDIR=/var/nfs_dir/dynemol' >> ~/.bashrc
echo 'source /opt/intel/oneapi/setvars.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc
