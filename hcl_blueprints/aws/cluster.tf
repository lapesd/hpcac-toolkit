variable "availability_zone" {}

variable "private_rsa_key_path" {}
variable "public_rsa_key_path" {}
variable "public_key_name" {}

variable "entrypoint_ami" {}
variable "entrypoint_ebs" {}
variable "entrypoint_rbs" {}
variable "entrypoint_instance_type" {}

variable "worker_count" {}
variable "worker_ami" {}
variable "worker_ebs" {}
variable "worker_rbs" {}
variable "worker_instance_type" {}
variable "worker_spot_price" {}


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

resource "aws_instance" "entrypoint_node" {
  ami             = var.entrypoint_ami
  instance_type   = var.entrypoint_instance_type
  security_groups = [aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_mpi.id]
  subnet_id       = aws_subnet.cluster_subnet.id
  key_name        = aws_key_pair.deployer_key.key_name
  root_block_device {
    delete_on_termination = "true"
    volume_size           = var.entrypoint_rbs
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = var.entrypoint_ebs
  }
  private_ip = "10.0.0.10"
  depends_on = [aws_internet_gateway.cluster_ig]
  tags = {
    Name = "Entrypoint Node"
  }
}

resource "null_resource" "setup_entrypoint_node" {
  connection {
    type        = "ssh"
    host        = aws_instance.entrypoint_node.public_ip
    user        = "ec2-user"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "file" {
    source      = "./scripts/base_ompi_setup.sh"
    destination = "/tmp/base_ompi_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/base_ompi_setup.sh",
      "/tmp/base_ompi_setup.sh"
    ]
  }
  provisioner "file" {
    source      = "./scripts/entrypoint_nfs_setup.sh"
    destination = "/tmp/entrypoint_nfs_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/entrypoint_nfs_setup.sh",
      "/tmp/entrypoint_nfs_setup.sh"
    ]
  }
  provisioner "file" {
    source      = "./keys/id_rsa"
    destination = "/home/ec2-user/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = "./keys/id_rsa.pub"
    destination = "/home/ec2-user/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 600 /home/ec2-user/.ssh/id_rsa"
    ]
  }
}

resource "aws_spot_instance_request" "worker_node" {
  count           = var.worker_count
  ami             = var.worker_ami
  instance_type   = var.worker_instance_type
  spot_price      = var.worker_spot_price
  security_groups = [aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_mpi.id]
  subnet_id       = aws_subnet.cluster_subnet.id
  key_name        = aws_key_pair.deployer_key.key_name
  root_block_device {
    delete_on_termination = "true"
    volume_size           = "10"
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = "10"
  }
  spot_type                      = "one-time"
  instance_interruption_behavior = "terminate"
  wait_for_fulfillment           = "true"

  private_ip = "10.0.0.1${count.index + 1}"
  depends_on = [aws_internet_gateway.cluster_ig, aws_instance.entrypoint_node, null_resource.setup_entrypoint_node]
  monitoring = true
  tags = {
    Name = "Worker ${count.index + 1}"
  }
}

resource "null_resource" "setup_worker_nodes" {
  count = var.worker_count
  connection {
    type        = "ssh"
    host        = aws_spot_instance_request.worker_node[count.index].public_ip
    user        = "ec2-user"
    private_key = file(var.private_rsa_key_path)
  }
  provisioner "file" {
    source      = "./scripts/base_ompi_setup.sh"
    destination = "/tmp/base_ompi_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/base_ompi_setup.sh ${aws_key_pair.deployer_key.public_key}",
      "/tmp/base_ompi_setup.sh"
    ]
  }
  provisioner "file" {
    source      = "./scripts/worker_node_nfs_setup.sh"
    destination = "/tmp/worker_node_nfs_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/worker_node_nfs_setup.sh",
      "/tmp/worker_node_nfs_setup.sh 10.0.0.10"
    ]
  }
  provisioner "file" {
    source      = "./keys/id_rsa"
    destination = "/home/ec2-user/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = "./keys/id_rsa.pub"
    destination = "/home/ec2-user/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 600 /home/ec2-user/.ssh/id_rsa"
    ]
  }
}
