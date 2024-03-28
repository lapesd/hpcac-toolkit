#!/bin/bash

# Set the benchmarks variable
benchmarks=("is" "ep" "cg" "lu" "sp" "bt" "mg" "ft")

# Directory paths
myf_dir="/var/nfs_dir/my_files"
mpi_dir="$myf_dir/NPB3.4.2/NPB3.4-MPI"

# Change directory and extract files
cd "$myf_dir" || exit 1
tar -xvzf NPB3.4.2.tar.gz || exit 1
cd "$mpi_dir" || exit 1

# Create necessary directories if they don't exist
mkdir -p "$mpi_dir/bin" "$mpi_dir/logs" || exit 1

# Recompiling stuff for ARM
make clean

echo "### Running make."
for j in "${benchmarks[@]}"; do
        echo "##### Making $j."
        make "$j" CLASS="C" > /dev/null 2>&1 || exit 1
		echo "##### Made $j."
done

echo "### Running benchmarks."
for i in {1..5}; do
	echo "##### Round $i."
	
	for bm in "${benchmarks[@]}"; do 
		echo "####### Benchmark $bm round $i."
		mpiexec -np 4 "$mpi_dir/bin/$bm.C.x" >> "$mpi_dir/logs/$bm.log" 2> /dev/null || exit 1
		echo "####### Finished $bm round $i."
	done
	
	echo "##### Finished round $i."

done

echo "### Tests completed successfully."
