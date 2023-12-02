# Use "ami-0628a44f475115373" as base (it's the IntelMPI base image configured for EFA)
sudo fallocate -l 1G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
sudo swapon -s

git clone https://github.com/spack/spack.git

echo 'source ~/spack/share/spack/setup-env.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc

spack spec chameleon@1.2.0+cuda+fxt+mpi^intel-mpi
spack install -v cuda@11.8.0%gcc@11.4.1~allow-unsupported-compilers~dev
spack install -v --keep-stage chameleon@1.2.0+cuda+fxt+mpi^intel-mpi
