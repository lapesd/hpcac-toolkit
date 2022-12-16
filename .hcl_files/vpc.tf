resource "aws_vpc" "cluster_vpc" {
  cidr_block = "10.0.0.0/16"
  instance_tenancy = "default"
  enable_dns_support = true
  enable_dns_hostnames = true
}

resource "aws_subnet" "cluster_subnet" {
  vpc_id = aws_vpc.cluster_vpc.id
  cidr_block = "10.0.0.0/20"
  availability_zone = "us-east-1a"
  map_public_ip_on_launch = true
}

resource "aws_internet_gateway" "cluster_ig" {
  vpc_id = aws_vpc.cluster_vpc.id
}

resource "aws_route_table" "cluster_rt" {
  vpc_id = aws_vpc.cluster_vpc.id
}

resource "aws_route" "cluster_r" {
  route_table_id = aws_route_table.cluster_rt.id
  destination_cidr_block = "0.0.0.0/0"
  gateway_id = aws_internet_gateway.cluster_ig.id
}

resource "aws_route_table_association" "cluster_rta" {
  subnet_id = aws_subnet.cluster_subnet.id
  route_table_id = aws_route_table.cluster_rt.id
}

resource "aws_security_group" "allow_ssh" {
  name = "allow_ssh"
  description = "Allow SSH traffic"
  vpc_id = aws_vpc.cluster_vpc.id

  ingress {
    from_port = 22
    to_port = 22
    protocol = "tcp"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }

  egress {
    from_port = 0
    to_port = 0
    protocol = "-1"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }
}

resource "aws_security_group" "allow_nfs" {
  name = "allow_nfs"
  description = "Allow NFS traffic"
  vpc_id = aws_vpc.cluster_vpc.id

  ingress {
    from_port = 2049
    to_port = 2049
    protocol = "tcp"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }

  egress {
    from_port = 0
    to_port = 0
    protocol = "-1"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }
}

resource "aws_security_group" "allow_mpi" {
  name = "allow_mpi"
  description = "Allow MPI traffic"
  vpc_id = aws_vpc.cluster_vpc.id

  ingress {
    from_port = 0
    to_port = 65535
    protocol = "tcp"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }

  egress {
    from_port = 0
    to_port = 65535
    protocol = "tcp"
    cidr_blocks = [
      "0.0.0.0/0"
    ]
  }
}
