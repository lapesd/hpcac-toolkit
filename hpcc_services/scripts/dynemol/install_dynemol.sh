# Run this on top of the base Amazon Linux 2 image
# git clone git@github.com:vanderlei-filho/Dynemol.git
git clone git@github.com:lgcrego/Dynemol.git

cd Dynemol
make

echo 'export DYNEMOLDIR=/home/ec2-user/Dynemol' >> ~/.bashrc
echo 'export DYNEMOLWORKDIR=/var/nfs_dir/dynemol' >> ~/.bashrc
source ~/.bashrc
