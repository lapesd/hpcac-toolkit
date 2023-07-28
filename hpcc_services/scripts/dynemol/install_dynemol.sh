# Run this on top of the base Amazon Linux 2 image
git clone git@github.com:lgcrego/Dynemol.git

cd Dynemol
make

# Setup Intel OneAPI environment initialization
echo 'export DYNEMOLDIR=/home/ec2-user/Dynemol' >> ~/.bashrc
echo 'export DYNEMOLWORKDIR=/var/nfs_dir/dynemol' >> ~/.bashrc
echo 'source /opt/intel/oneapi/setvars.sh > /dev/null' >> ~/.bashrc
source ~/.bashrc

# Disable HT
for cpunum in $(cat /sys/devices/system/cpu/cpu*/topology/thread_siblings_list | cut -s -d, -f2- | tr ',' '\n' | sort -un)
do
	echo 0 > /sys/devices/system/cpu/cpu$cpunum/online
done
