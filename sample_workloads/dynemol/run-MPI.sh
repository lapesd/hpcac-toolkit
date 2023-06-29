#!/bin/bash
#SBATCH --nodes=1            
#SBATCH --ntasks-per-node=3
#SBATCH --ntasks=3
#SBATCH --cpus-per-task=8
#SBATCH -p alpha
#SBATCH --time=00:35:00

#export DYNEMOLWORKDIR=$(pwd)
#export DYNEMOLDIR=/scratch/luis/development/MPI

export OMP_NUM_THREADS=$SLURM_CPUS_PER_TASK

export I_MPI_PMI_LIBRARY=/opt/intel/oneapi/mpi/2023.1.0/lib/libpmi.so

source /opt/intel/oneapi/setvars.sh > /dev/null

mpirun $DYNEMOLDIR/dynemol
