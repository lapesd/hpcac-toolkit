# DOCS: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/efa-start.html

sudo yum -y install kernel-{devel,headers}-$(uname -r)
sudo yum -y install git gcc make cmake rpm-build

# Install the following if you are going to use OpenMPI:
sudo yum -y install libevent libgfortran hwloc


git clone git@github.com:vanderlei-filho/efa-installer.git
cd efa-installer

# If we use the --minimal flag, you must install Intel MPI
sudo ./efa_installer.sh -y --minimal  # remove the --minimal flag if you want to use OpenMPI

# Disable ptrace
sudo sysctl -w kernel.yama.ptrace_scope=0

# Set MPI to use EFA libfabric
echo 'export I_MPI_OFI_LIBRARY_INTERNAL=0' >> ~/.bashrc
echo 'export I_MPI_OFI_PROVIDER=efa' >> ~/.bashrc
echo 'export FI_PROVIDER=efa' >> ~/.bashrc
echo 'export I_MPI_DEBUG=5' >> ~/.bashrc
echo 'export I_MPI_FABRICS=ofi' >> ~/.bashrc

source ~/.bashrc
