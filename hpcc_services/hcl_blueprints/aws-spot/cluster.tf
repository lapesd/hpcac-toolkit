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
variable "worker_spot_price" {}

variable "experiment_tag" {}


resource "aws_vpc" "cluster_vpc" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true
  tags = {
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_subnet" "cluster_subnet" {
  vpc_id                  = aws_vpc.cluster_vpc.id
  cidr_block              = "10.0.0.0/20"
  availability_zone       = var.availability_zone
  map_public_ip_on_launch = true
  tags = {
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_internet_gateway" "cluster_ig" {
  vpc_id = aws_vpc.cluster_vpc.id
  tags = {
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_route_table" "cluster_rt" {
  vpc_id = aws_vpc.cluster_vpc.id
  tags = {
    "cost_allocation_tag" = var.experiment_tag
  }
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
    tags = {
      Name                  = "Master RBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = var.master_ebs
    tags = {
      Name                  = "Master EBS",
      "cost_allocation_tag" = var.experiment_tag
    }
  }
  private_ip = "10.0.0.10"
  depends_on = [aws_internet_gateway.cluster_ig]
  tags = {
    Name                  = "Master Node"
    "cost_allocation_tag" = var.experiment_tag
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

  # Setup NFS server
  provisioner "file" {
    source      = "../scripts/nfs/nfs_server_setup.sh"
    destination = "/tmp/nfs_server_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs_server_setup.sh",
      "/tmp/nfs_server_setup.sh"
    ]
  }

  # RUN cluster-init script
  provisioner "file" {
    source      = "../cluster_init.sh"
    destination = "/tmp/cluster_init.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/cluster_init.sh",
      "/tmp/cluster_init.sh"
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
    tags = {
      Name                  = "Worker ${count.index + 1} RBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }
  ebs_block_device {
    delete_on_termination = "true"
    device_name           = "/dev/sdh"
    volume_size           = "10"
    tags = {
      Name                  = "Worker ${count.index + 1} EBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }
  spot_type                      = "one-time"
  instance_interruption_behavior = "terminate"
  wait_for_fulfillment           = "true"

  private_ip = "10.0.0.1${count.index + 1}"
  depends_on = [aws_internet_gateway.cluster_ig, aws_instance.master_node, null_resource.setup_master_node]
  monitoring = true
  tags = {
    Name                  = "Worker ${count.index + 1}"
    "cost_allocation_tag" = var.experiment_tag
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

  # Setup NFS client access
  provisioner "file" {
    source      = "../scripts/nfs/nfs_client_setup.sh"
    destination = "/tmp/nfs_client_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs_client_setup.sh",
      "/tmp/nfs_client_setup.sh 10.0.0.10"
    ]
  }

  # RUN cluster-init script
  provisioner "file" {
    source      = "../cluster_init.sh"
    destination = "/tmp/cluster_init.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/cluster_init.sh",
      "/tmp/cluster_init.sh"
    ]
  }
}

output "master_node_public_ip" {
  description = "Master Node public IP"
  value       = aws_instance.master_node.public_ip
}
