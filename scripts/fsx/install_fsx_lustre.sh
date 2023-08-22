git clone git://git.whamcloud.com/fs/lustre-release.git
cd lustre-release
git checkout 2.12.56
sh autogen.sh
./configure -j32 --with-mlx4-mod --with-user_access-mod --with-user_mad-mod --with-addr_trans-mod --with-pa-mr --with-core-mod
make rpms
