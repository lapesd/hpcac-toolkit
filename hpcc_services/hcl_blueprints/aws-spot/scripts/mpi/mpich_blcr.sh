sudo yum groupinstall "Development Tools" -y
sudo yum install kernel-devel openssl-devel -y

wget https://crd.lbl.gov/assets/Uploads/FTG/Projects/CheckpointRestart/downloads/blcr-0.8.5.tar.gz
tar -xf blcr-0.8.5.tar.gz
cd blcr-0.8.5
./configure
make
sudo make install
sudo insmod /lib/modules/$(uname -r)/extra/blcr/blcr_imports.ko
./blcr-testsuite


# INSTALL MPICH
wget https://www.mpich.org/static/downloads/4.1.1/mpich-4.1.1.tar.gz
tar xzf mpich-4.1.1.tar.gz
cd mpich-4.1.1

--with-blcr=<BLCR_INSTALL_DIR> LD_LIBRARY_PATH=<BLCR_INSTALL_DIR>/lib
./configure --with-blcr= --with-device=ch3:sock 2>&1 | tee c.txt
