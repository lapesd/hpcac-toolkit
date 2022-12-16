# Reference terraform.tfvars variables
variable "region" {}
variable "worker_instance_type" {}
variable "worker_count" {}
variable "worker_os_id" {}
variable "head_instance_type" {}
variable "head_os_id" {}
variable "private_rsa_key_path" {}
variable "public_rsa_key_path" {}
variable "public_key_name" {}
variable "password" {}

resource "vultr_vpc" "cluster_vpc" {
  description    = "cluster-vpc"
  region         = "dfw"
  v4_subnet      = "10.3.96.0"
  v4_subnet_mask = 20
}

resource "vultr_ssh_key" "deployer_key" {
  name    = var.public_key_name
  ssh_key = file(var.public_rsa_key_path)
}

# -----------------------------------------------------------------------------
#  HEAD INSTANCE SETUP
#

resource "vultr_instance" "cluster_head" {
  region          = var.region
  plan            = var.head_instance_type
  os_id           = var.head_os_id
  label           = "cluster-head"
  backups         = "disabled"
  ddos_protection = false
  vpc_ids         = [vultr_vpc.cluster_vpc.id]
  ssh_key_ids     = [vultr_ssh_key.deployer_key.id]
  hostname        = "cluster-head"
}

resource "null_resource" "setup_head_ssh" {
  depends_on = [vultr_instance.cluster_head]
  connection {
    type        = "ssh"
    host        = vultr_instance.cluster_head.main_ip
    user        = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "mkdir -p /root/.ssh"
    ]
  }
  provisioner "file" {
    source      = "./keys/id_rsa"
    destination = "/root/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = "./keys/id_rsa.pub"
    destination = "/root/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 0600 /root/.ssh/id_rsa",
      "rm /etc/ssh/ssh_config",
      "touch /etc/ssh/ssh_config",
      "sh -c \"echo 'Host *' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        StrictHostKeyChecking no' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        UserKnownHostsFile=/dev/null' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        PasswordAuthentication no' >> /etc/ssh/ssh_config\"",
      "touch /root/.ssh/authorized_keys",
      "sh -c \"echo '${vultr_ssh_key.deployer_key.ssh_key}' >> /root/.ssh/authorized_keys\"",
      "chmod 0600 /root/.ssh/authorized_keys",
      "systemctl restart sshd.service"
    ]
  }
}

resource "vultr_block_storage" "cluster_block_storage" {
  size_gb              = 40
  block_type           = "storage_opt"
  region               = var.region
  attached_to_instance = vultr_instance.cluster_head.id
  live                 = true
}

resource "null_resource" "setup_head_nfs_server" {
  depends_on = [vultr_block_storage.cluster_block_storage, null_resource.setup_head_ssh]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_head.main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "sudo yum -y update",
      "yum remove -y firewalld",
      "yum install -y wget perl gcc gcc-c++ gcc-gfortran nfs-utils git autoconf automake m4 libtool flex openssl-devel iptables-services",
      "yum clean all",
      "rm -rf /var/cache/yum",
      "mkfs -t ext4 /dev/vdb",
      "mkdir /var/nfs_dir",
      "mount /dev/vdb /var/nfs_dir",
      "chown nfsnobody:nfsnobody /var/nfs_dir",
      "chmod 755 /var/nfs_dir",
      "systemctl unmask rpcbind && sudo systemctl enable rpcbind && sudo systemctl start rpcbind",
      "systemctl enable nfs-server && sudo systemctl start nfs-server",
      "systemctl enable nfs-lock && sudo systemctl start nfs-lock",
      "systemctl enable nfs-idmap && sudo systemctl start nfs-idmap",
      "systemctl enable rpc-statd && sudo systemctl start rpc-statd",
      "sh -c \"echo '/var/nfs_dir      *(rw,sync,no_root_squash)' >> /etc/exports\"",
      "systemctl restart nfs-server",
      "chmod ugo+rwx /var/nfs_dir"
    ]
  }
}

resource "null_resource" "setup_head_firewall" {
  count = var.worker_count
  depends_on = [null_resource.setup_head_nfs_server]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_head.main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "systemctl start iptables",
      "systemctl enable iptables",
      "iptables -L -n",
      "iptables -F",
      "iptables -X",
      "iptables -t nat -F",
      "iptables -t nat -X",
      "iptables -t mangle -F",
      "iptables -t mangle -X",
      "iptables -P INPUT ACCEPT",
      "iptables -P FORWARD ACCEPT",
      "iptables -P OUTPUT ACCEPT",
      "/usr/libexec/iptables/iptables.init save",
      "systemctl restart iptables",
      "iptables -L -n"
    ]
  }
}

resource "null_resource" "setup_head_mpi" {
  count = var.worker_count
  depends_on = [null_resource.setup_head_firewall]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_head.main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "git clone --recursive https://github.com/open-mpi/ompi.git",
      "cd ompi || exit",
      "sudo ./autogen.pl --no-oshmem",
      "sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g'",
      "sudo make install && git clean -fdx && rm -rf 3rd-party"
    ]
  }
}

#resource "null_resource" "setup_head_env_varsx" {
#  count = var.worker_count
#  depends_on = [null_resource.setup_head_mpi]
#  connection {
#    type = "ssh"
#    host = vultr_instance.cluster_head.main_ip
#    user = "root"
#    private_key = file(var.private_rsa_key_path)
#  }
#  provisioner "remote-exec" {
#    inline = [
#      "echo 'export OMPI_ALLOW_RUN_AS_ROOT=1' >> $HOME/.bashrc",
#      "echo 'export OMPI_ALLOW_RUN_AS_ROOT_CONFIRM=1' >> $HOME/.bashrc"
#    ]
#  }
#}

# -----------------------------------------------------------------------------
#  WORKER INSTANCES SETUP
#

resource "vultr_instance" "cluster_worker" {
  count           = var.worker_count
  region          = var.region
  plan            = var.worker_instance_type
  os_id           = var.worker_os_id
  label           = "cluster-worker-${count.index + 1}"
  backups         = "disabled"
  ddos_protection = false
  vpc_ids         = [vultr_vpc.cluster_vpc.id]
  ssh_key_ids     = [vultr_ssh_key.deployer_key.id]
  hostname        = "cluster-worker-${count.index + 1}"
}

resource "null_resource" "setup_worker_ssh" {
  count      = var.worker_count
  depends_on = [vultr_instance.cluster_worker]
  connection {
    type        = "ssh"
    host        = vultr_instance.cluster_worker[count.index].main_ip
    user        = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "file" {
    source      = "./keys/id_rsa"
    destination = "/root/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = "./keys/id_rsa.pub"
    destination = "/root/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 0600 /root/.ssh/id_rsa",
      "rm /etc/ssh/ssh_config",
      "touch /etc/ssh/ssh_config",
      "sh -c \"echo 'Host *' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        StrictHostKeyChecking no' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        UserKnownHostsFile=/dev/null' >> /etc/ssh/ssh_config\"",
      "sh -c \"echo '        PasswordAuthentication no' >> /etc/ssh/ssh_config\"",
      "touch /root/.ssh/authorized_keys",
      "sh -c \"echo '${vultr_ssh_key.deployer_key.ssh_key}' >> /root/.ssh/authorized_keys\"",
      "chmod 0600 /root/.ssh/authorized_keys",
      "systemctl restart sshd.service"
    ]
  }
}

resource "null_resource" "setup_workers_nfs" {
  count = var.worker_count
  depends_on = [null_resource.setup_head_nfs_server, null_resource.setup_worker_ssh]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_worker[count.index].main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "sudo yum -y update",
      "yum remove -y firewalld",
      "yum install -y wget perl gcc gcc-c++ gcc-gfortran nfs-utils git autoconf automake m4 libtool flex openssl-devel iptables-services",
      "yum clean all",
      "rm -rf /var/cache/yum",
      "mkdir -p /var/nfs_dir",
      "mount -t nfs ${vultr_instance.cluster_head.internal_ip}:/var/nfs_dir /var/nfs_dir",
      "chmod ugo+rwx /var/nfs_dir"
    ]
  }
}

resource "null_resource" "setup_workers_firewall" {
  count = var.worker_count
  depends_on = [null_resource.setup_workers_nfs]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_worker[count.index].main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "systemctl start iptables",
      "systemctl enable iptables",
      "iptables -L -n",
      "iptables -F",
      "iptables -X",
      "iptables -t nat -F",
      "iptables -t nat -X",
      "iptables -t mangle -F",
      "iptables -t mangle -X",
      "iptables -P INPUT ACCEPT",
      "iptables -P FORWARD ACCEPT",
      "iptables -P OUTPUT ACCEPT",
      "/usr/libexec/iptables/iptables.init save",
      "systemctl restart iptables",
      "iptables -L -n"
    ]
  }
}

resource "null_resource" "setup_workers_mpi" {
  count = var.worker_count
  depends_on = [null_resource.setup_workers_firewall]
  connection {
    type = "ssh"
    host = vultr_instance.cluster_worker[count.index].main_ip
    user = "root"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "remote-exec" {
    inline = [
      "git clone --recursive https://github.com/open-mpi/ompi.git",
      "cd ompi || exit",
      "sudo ./autogen.pl --no-oshmem",
      "sudo ./configure --disable-io-romio --enable-debug CFLAGS='-O0 -g'",
      "sudo make install && git clean -fdx && rm -rf 3rd-party"
    ]
  }
}

#resource "null_resource" "setup_workers_env_vars" {
#  count = var.worker_count
#  depends_on = [null_resource.setup_workers_mpi]
#  connection {
#    type = "ssh"
#    host = vultr_instance.cluster_worker[count.index].main_ip
#    user = "root"
#    private_key = file(var.private_rsa_key_path)
#  }
#  provisioner "remote-exec" {
#    inline = [
#      "echo 'export OMPI_ALLOW_RUN_AS_ROOT=1' >> $HOME/.bashrc",
#      "echo 'export OMPI_ALLOW_RUN_AS_ROOT_CONFIRM=1' >> $HOME/.bashrc"
#    ]
#  }
#}

output "cluster_head_public_ip" {
  description = "Entrypoint node public IP address"
  value       = vultr_instance.cluster_head.main_ip
}

output "cluster_head_private_ip" {
  description = "Entrypoint node private IP address"
  value       = vultr_instance.cluster_head.internal_ip
}