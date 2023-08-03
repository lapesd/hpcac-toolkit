cd /var/nfs_dir
wget https://mvapich.cse.ohio-state.edu/download/mvapich/osu-micro-benchmarks-7.2.tar.gz

tar xf osu-micro-benchmarks-7.2.tar.gz 
cd osu-micro-benchmarks-7.2

./configure --prefix=/var/nfs_dir/osu-micro-benchmarks-7.2/install CC=mpicc CXX=mpicxx
make -j 4
make install

# mpirun -n 2 -ppn 1 -hosts 10.0.0.10,10.0.0.11 ./osu_latency