#!/bin/bash
#SBATCH --nodes=1                      
#SBATCH --ntasks-per-node=4            
#SBATCH --ntasks=3
#SBATCH --cpus-per-task=8
#SBATCH -p alpha
#SBATCH --exclude=fisica-01,fisica-06,fisica-07
#SBATCH --time=02:35:00

export DYNEMOLWORKDIR=$(pwd)/$1
export DYNEMOLDIR=/scratch/luis/development/MPI

export OMP_NUM_THREADS=$SLURM_CPUS_PER_TASK

export I_MPI_PMI_LIBRARY=/opt/intel/oneapi/mpi/2021.6.0/lib/libpmi.so

source /opt/intel/oneapi/setvars.sh > /dev/null

cd $(pwd)/$1

mpirun $DYNEMOLDIR/dynemol
