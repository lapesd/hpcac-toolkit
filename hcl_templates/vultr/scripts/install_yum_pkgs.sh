# Apply OS packages updates
sudo yum -y update

# Remove firewalld
sudo yum remove -y firewalld

# Install dependencies
sudo yum install -y \
  wget \
  perl \
  gcc \
  gcc-c++ \
  gcc-gfortran \
  nfs-utils \
  git \
  autoconf \
  automake \
  m4 \
  libtool \
  flex \
  openssl-devel \
  iptables-services

# Clean yum cache and metadata
sudo yum clean all
sudo rm -rf /var/cache/yum
