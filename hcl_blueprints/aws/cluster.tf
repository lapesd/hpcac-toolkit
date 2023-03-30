variable "availability_zone" {}

variable "private_rsa_key_path" {}
variable "public_rsa_key_path" {}
variable "public_key_name" {}

variable "master_ami" {}
variable "master_ebs" {}
variable "master_rbs" {}
variable "master_instance_type" {}

variable "worker_count" {}
variable "worker_ami" {}
variable "worker_ebs" {}
variable "worker_rbs" {}
variable "worker_instance_type" {}


resource "aws_vpc" "cluster_vpc" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true
}

resource "aws_subnet" "cluster_subnet" {
  vpc_id                  = aws_vpc.cluster_vpc.id
  cidr_block              = "10.0.0.0/20"
  availability_zone       = var.availability_zone
  map_public_ip_on_launch = true
}

resource "aws_internet_gateway" "cluster_ig" {
  vpc_id = aws_vpc.cluster_vpc.id
}

resource "aws_route_table" "cluster_rt" {
  vpc_id = aws_vpc.cluster_vpc.id
}

resource "aws_route" "cluster_r" {
  route_table_id         = aws_route_table.cluster_rt.id
  destination_cidr_block = "0.0.0.0/0"
  gateway_id             = aws_internet_gateway.cluster_ig.id
}

resource "aws_route_table_association" "cluster_rt_association" {
  subnet_id      = aws_subnet.cluster_subnet.id
  route_table_id = aws_route_table.cluster_rt.id
}

resource "aws_security_group" "allow_ssh" {
  name        = "allow_ssh"
  description = "Allow SSH traffic"
  vpc_id      = aws_vpc.cluster_vpc.id
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_security_group" "allow_nfs" {
  name        = "allow_nfs"
  description = "Allow NFS traffic"
  vpc_id      = aws_vpc.cluster_vpc.id
  ingress {
    from_port   = 2049
    to_port     = 2049
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_security_group" "allow_mpi" {
  name        = "allow_mpi"
  description = "Allow MPI traffic"
  vpc_id      = aws_vpc.cluster_vpc.id
  ingress {
    from_port   = 0
    to_port     = 65535
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
  egress {
    from_port   = 0
    to_port     = 65535
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_key_pair" "deployer_key" {
  key_name   = var.public_key_name
  public_key = file(var.public_rsa_key_path)
}

resource "aws_instance" "master_node" {
  ami             = var.master_ami
  instance_type   = var.master_instance_type
  security_groups = [aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_mpi.id]
  subnet_id       = aws_subnet.cluster_subnet.id
  key_name        = aws_key_pair.deployer_key.key_name
  root_block_device {
    delete_on_termination = "true"
    volume_size           = var.master_rbs
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = var.master_ebs
  }
  private_ip = "10.0.0.10"
  depends_on = [aws_internet_gateway.cluster_ig]
  tags = {
    Name = "Master Node"
  }
}

resource "null_resource" "setup_master_node" {
  connection {
    type        = "ssh"
    host        = aws_instance.master_node.public_ip
    user        = "ec2-user"
    private_key = file(var.private_rsa_key_path)
  }

  # Copy SSH keys
  provisioner "file" {
    source      = var.private_rsa_key_path
    destination = "/home/ec2-user/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = var.public_rsa_key_path
    destination = "/home/ec2-user/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 600 /home/ec2-user/.ssh/id_rsa"
    ]
  }

  /*
  # Basic EC2 configuration
  provisioner "file" {
    source      = "./scripts/base_setup.sh"
    destination = "/tmp/base_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/base_setup.sh ${aws_key_pair.deployer_key.public_key}",
      "/tmp/base_setup.sh"
    ]
  }
  */

  # Setup NFS server
  provisioner "file" {
    source      = "./scripts/nfs/nfs_server_setup.sh"
    destination = "/tmp/nfs_server_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs_server_setup.sh",
      "/tmp/nfs_server_setup.sh"
    ]
  }

  /*
  # Install OpenMPI with ULFM support
  provisioner "file" {
    source      = "./scripts/mpi/install_ulfm_ompi.sh"
    destination = "/tmp/install_ulfm_ompi.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_ulfm_ompi.sh",
      "/tmp/install_ulfm_ompi.sh"
    ]
  }
  */

  /*
  # Install MVAPICH
  provisioner "file" {
    source      = "./scripts/mpi/install_tcp_mvapich.sh"
    destination = "/tmp/install_tcp_mvapich.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_tcp_mvapich.sh",
      "/tmp/install_tcp_mvapich.sh"
    ]
  }
  */

  /*
  # Install Singularity
  provisioner "file" {
    source      = "./scripts/singularity/install_singularity.sh"
    destination = "/tmp/install_singularity.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_singularity.sh",
      "/tmp/install_singularity.sh"
    ]
  }
  */
}

resource "aws_instance" "worker_node" {
  count           = var.worker_count
  ami             = var.worker_ami
  instance_type   = var.worker_instance_type
  security_groups = [aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_mpi.id]
  subnet_id       = aws_subnet.cluster_subnet.id
  key_name        = aws_key_pair.deployer_key.key_name
  root_block_device {
    delete_on_termination = "true"
    volume_size           = var.worker_rbs
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = var.worker_ebs
  }
  private_ip = "10.0.0.1${count.index + 1}"
  depends_on = [aws_internet_gateway.cluster_ig, aws_instance.master_node, null_resource.setup_master_node]
  tags = {
    Name = "Worker ${count.index + 1}"
  }
}

resource "null_resource" "setup_worker_nodes" {
  count = var.worker_count
  connection {
    type        = "ssh"
    host        = aws_instance.worker_node[count.index].public_ip
    user        = "ec2-user"
    private_key = file(var.private_rsa_key_path)
  }

  # Copy SSH keys
  provisioner "file" {
    source      = var.private_rsa_key_path
    destination = "/home/ec2-user/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = var.public_rsa_key_path
    destination = "/home/ec2-user/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 600 /home/ec2-user/.ssh/id_rsa"
    ]
  }

  /*
  # Basic EC2 configuration
  provisioner "file" {
    source      = "./scripts/base_setup.sh"
    destination = "/tmp/base_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/base_setup.sh ${aws_key_pair.deployer_key.public_key}",
      "/tmp/base_setup.sh"
    ]
  }
  */

  # Setup NFS client access
  provisioner "file" {
    source      = "./scripts/nfs/nfs_client_setup.sh"
    destination = "/tmp/nfs_client_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs_client_setup.sh",
      "/tmp/nfs_client_setup.sh 10.0.0.10"
    ]
  }

  /*
  # Install OpenMPI with ULFM support
  provisioner "file" {
    source      = "./scripts/mpi/install_ulfm_ompi.sh"
    destination = "/tmp/install_ulfm_ompi.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_ulfm_ompi.sh",
      "/tmp/install_ulfm_ompi.sh"
    ]
  }
  */

  /*
  # Install MVAPICH
  provisioner "file" {
    source      = "./scripts/mpi/install_tcp_mvapich.sh"
    destination = "/tmp/install_tcp_mvapich.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_tcp_mvapich.sh",
      "/tmp/install_tcp_mvapich.sh"
    ]
  }
  */

  /*
  # Install Singularity
  provisioner "file" {
    source      = "./scripts/singularity/install_singularity.sh"
    destination = "/tmp/install_singularity.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/install_singularity.sh",
      "/tmp/install_singularity.sh"
    ]
  }
  */
}

output "master_node_public_ip" {
  description = "Master Node public IP"
  value       = aws_instance.master_node.public_ip
}

output "worker_node_public_ips" {
  description = "Worker Node public IPs"
  value       = [for instance in aws_instance.worker_node : instance.public_ip]
}
