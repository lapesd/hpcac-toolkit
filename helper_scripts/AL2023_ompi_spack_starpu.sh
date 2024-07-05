# Install AL2023_install_cuda_toolkit.sh first

# Install OpenMPI
sudo yum install -y openmpi-devel
sudo ln -s /usr/include/openmpi-x86_64 /usr/lib64/openmpi/include

# Install Spack + StarPU
git clone https://github.com/spack/spack.git
cd spack
git pull
git checkout e96f31c29d3408e6421d277728272c7c037c199b
git apply patchStarpu.patch  # you need the patchStarpu.patch file for this.
spack uninstall --dependents --yes-to-all openmpi openblas
spack install -v chameleon@1.2.0+cuda+fxt+mpi^cuda@11.8.0^fxt@0.3.14^openblas@0.3.24^openmpi@4.1.2^starpu@1.4.1+cuda+fxt+mpi
