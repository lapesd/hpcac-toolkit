variable "availability_zone" {}

variable "private_rsa_key_path" {}
variable "public_rsa_key_path" {}
variable "public_key_name" {}

variable "master_ami" {}
variable "master_instance_type" {}
variable "master_rbs_size" {}
variable "master_rbs_type" {}
variable "master_rbs_iops" {}

variable "worker_count" {}
variable "worker_ami" {}
variable "worker_instance_type" {}
variable "worker_rbs_size" {}
variable "worker_rbs_type" {}
variable "worker_rbs_iops" {}

variable "use_spot" {}
variable "use_efa" {}
variable "use_nfs" {}
variable "use_fsx" {}
variable "spot_maximum_rate" {}

variable "experiment_tag" {}
variable "instance_username" {}


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

resource "aws_security_group" "allow_lustre" {
  name        = "allow_lustre"
  description = "Allow Lustre FSx traffic"
  vpc_id      = aws_vpc.cluster_vpc.id

  ingress {
    from_port   = 988
    to_port     = 988
    protocol    = "tcp"
    self        = true
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

resource "aws_security_group" "allow_all" {
  name        = "allow_all"
  description = "Allow all traffic"
  vpc_id      = aws_vpc.cluster_vpc.id

  ingress {
    from_port = 0
    to_port   = 0
    protocol  = "-1" # All protocols
    self      = true
  }

  egress {
    from_port = 0
    to_port   = 0
    protocol  = "-1" # All protocols
    self      = true
  }
}

resource "aws_key_pair" "deployer_key" {
  key_name   = var.public_key_name
  public_key = file(var.public_rsa_key_path)
}

resource "aws_placement_group" "cluster_pg" {
  name     = "cluster-placement-group"
  strategy = "cluster"
  tags = {
    Name                  = "Cluster Placement Group"
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_network_interface" "master_node_eni" {
  subnet_id       = aws_subnet.cluster_subnet.id
  private_ips     = ["10.0.0.10"]
  security_groups = [aws_security_group.allow_all.id, aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_lustre.id]
  interface_type  = var.use_efa ? "efa" : null
  tags = {
    Name                  = "Master Node ENI"
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_instance" "master_node" {
  ami             = var.master_ami
  instance_type   = var.master_instance_type
  key_name        = aws_key_pair.deployer_key.key_name
  placement_group = var.use_efa ? aws_placement_group.cluster_pg.name : null
  depends_on      = [aws_internet_gateway.cluster_ig, aws_placement_group.cluster_pg, aws_network_interface.master_node_eni]
  tags = {
    Name                  = "Master Node"
    "cost_allocation_tag" = var.experiment_tag
  }

  root_block_device {
    volume_type           = var.master_rbs_type
    volume_size           = var.master_rbs_size
    delete_on_termination = true
    iops                  = var.master_rbs_iops
    tags = {
      Name                  = "Master RBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }

  network_interface {
    device_index         = 0
    network_interface_id = aws_network_interface.master_node_eni.id
  }

  timeouts {
    create = "30m"
    update = "30m"
    delete = "30m"
  }
}

resource "null_resource" "setup_master_node" {
  connection {
    type        = "ssh"
    host        = aws_instance.master_node.public_ip
    user        = var.instance_username
    private_key = file(var.private_rsa_key_path)
  }

  # Copy SSH keys
  provisioner "file" {
    source      = var.private_rsa_key_path
    destination = "/home/${var.instance_username}/.ssh/id_rsa"
  }
  provisioner "file" {
    source      = var.public_rsa_key_path
    destination = "/home/${var.instance_username}/.ssh/id_rsa.pub"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod 600 /home/${var.instance_username}/.ssh/id_rsa"
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
      "/tmp/cluster_init.sh",
    ]
  }

  # DISABLE HT
  provisioner "remote-exec" {
    inline = [
      "sudo for cpunum in $(cat /sys/devices/system/cpu/cpu*/topology/thread_siblings_list | cut -s -d, -f2- | tr ',' '\n' | sort -un); do echo 0 > /sys/devices/system/cpu/cpu$cpunum/online; done",
    ]
  }
}

resource "null_resource" "setup_master_node_nfs" {
  count = var.use_nfs ? 1 : 0
  connection {
    type        = "ssh"
    host        = aws_instance.master_node.public_ip
    user        = var.instance_username
    private_key = file(var.private_rsa_key_path)
  }
  depends_on    = [null_resource.setup_master_node]

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
}

resource "aws_network_interface" "worker_node_eni" {
  count           = var.worker_count
  subnet_id       = aws_subnet.cluster_subnet.id
  private_ips     = ["10.0.0.1${count.index + 1}"]
  security_groups = [aws_security_group.allow_all.id, aws_security_group.allow_ssh.id, aws_security_group.allow_nfs.id, aws_security_group.allow_lustre.id]
  interface_type  = var.use_efa ? "efa" : null
  tags = {
    Name                  = "Worker ${count.index + 1} ENI"
    "cost_allocation_tag" = var.experiment_tag
  }
}

resource "aws_instance" "worker_node" {
  count           = var.use_spot ? 0 : var.worker_count
  ami             = var.worker_ami
  instance_type   = var.worker_instance_type
  key_name        = aws_key_pair.deployer_key.key_name
  placement_group = var.use_efa ? aws_placement_group.cluster_pg.name : null
  depends_on      = [null_resource.setup_master_node, aws_network_interface.worker_node_eni]
  tags = {
    Name                  = "Worker ${count.index + 1}"
    "cost_allocation_tag" = var.experiment_tag
  }

  root_block_device {
    volume_type           = var.worker_rbs_type
    volume_size           = var.worker_rbs_size
    delete_on_termination = true
    iops                  = var.worker_rbs_iops
    tags = {
      Name                  = "Worker ${count.index + 1} RBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }

  network_interface {
    device_index         = 0
    network_interface_id = aws_network_interface.worker_node_eni[count.index].id
  }

  timeouts {
    create = "30m"
    update = "30m"
    delete = "30m"
  }
}

resource "aws_spot_instance_request" "spot_worker_node" {
  count                          = var.use_spot ? var.worker_count : 0
  availability_zone              = var.availability_zone
  ami                            = var.worker_ami
  instance_type                  = var.worker_instance_type
  spot_price                     = var.spot_maximum_rate
  key_name                       = aws_key_pair.deployer_key.key_name
  placement_group                = var.use_efa ? aws_placement_group.cluster_pg.name : null
  depends_on                     = [null_resource.setup_master_node, aws_network_interface.worker_node_eni]
  spot_type                      = "one-time"
  instance_interruption_behavior = "terminate"
  wait_for_fulfillment           = "true"
  monitoring                     = true
  tags = {
    Name                  = "Worker ${count.index + 1}"
    "cost_allocation_tag" = var.experiment_tag
  }

  root_block_device {
    volume_type           = var.worker_rbs_type
    volume_size           = var.worker_rbs_size
    delete_on_termination = true
    iops                  = var.worker_rbs_iops
    tags = {
      Name                  = "Worker ${count.index + 1} RBS"
      "cost_allocation_tag" = var.experiment_tag
    }
  }
}

resource "aws_network_interface_attachment" "spot_worker_node_network_interface_attachment" {
  count                = var.use_spot ? var.worker_count : 0
  instance_id          = aws_spot_instance_request.spot_worker_node[count.index].spot_instance_id
  network_interface_id = aws_network_interface.worker_node_eni[count.index].id
  device_index         = 0
}

resource "null_resource" "setup_worker_nodes_ssh" {
  count = var.worker_count
  connection {
    type        = "ssh"
    host        = var.use_spot ? aws_spot_instance_request.spot_worker_node[count.index].public_ip : aws_instance.worker_node[count.index].public_ip
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

  # RUN cluster-init script
  provisioner "file" {
    source      = "../cluster_init.sh"
    destination = "/tmp/cluster_init.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/cluster_init.sh",
      "/tmp/cluster_init.sh",
    ]
  }

  # DISABLE HT
  provisioner "remote-exec" {
    inline = [
      "sudo for cpunum in $(cat /sys/devices/system/cpu/cpu*/topology/thread_siblings_list | cut -s -d, -f2- | tr ',' '\n' | sort -un); do echo 0 > /sys/devices/system/cpu/cpu$cpunum/online; done",
    ]
  }
}

resource "null_resource" "setup_worker_nodes_nfs" {
  count = var.use_nfs ? var.worker_count : 0
  connection {
    type        = "ssh"
    host        = var.use_spot ? aws_spot_instance_request.spot_worker_node[count.index].public_ip : aws_instance.worker_node[count.index].public_ip
    user        = "ec2-user"
    private_key = file(var.private_rsa_key_path)
  }
  depends_on    = [null_resource.setup_worker_nodes_ssh]

  # Setup NFS client access
  provisioner "file" {
    source      = "../scripts/nfs/nfs_client_setup.sh"
    destination = "/tmp/nfs_client_setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs_client_setup.sh",
      "/tmp/nfs_client_setup.sh"
    ]
  }
}

resource "aws_ec2_tag" "spot_worker_node_tags" {
  count = var.use_spot ? var.worker_count : 0

  resource_id = aws_spot_instance_request.spot_worker_node[count.index].spot_instance_id
  key         = "Name"
  value       = "Worker ${count.index + 1}"

  depends_on = [aws_spot_instance_request.spot_worker_node]
}

resource "aws_ec2_tag" "spot_worker_node_cost_tags" {
  count = var.use_spot ? var.worker_count : 0

  resource_id = aws_spot_instance_request.spot_worker_node[count.index].spot_instance_id
  key         = "cost_allocation_tag"
  value       = var.experiment_tag

  depends_on = [aws_spot_instance_request.spot_worker_node]
}

resource "aws_fsx_lustre_file_system" "lustre_fsx" {
  count = var.use_fsx ? 1 : 0
  storage_capacity    = 1200
  subnet_ids          = [aws_subnet.cluster_subnet.id]
  security_group_ids  = [aws_security_group.allow_lustre.id]
  depends_on = [null_resource.setup_worker_nodes_nfs]

  tags = {
    Name                  = "FSx Lustre Filesystem"
    "cost_allocation_tag" = var.experiment_tag
  }
}

output "fsx_lustre_dns_name" {
  description = "FSx Lustre filesystem DNS name"
  value = var.use_fsx ? aws_fsx_lustre_file_system.lustre_fsx[0].dns_name : "No Lustre FSx"
}

output "master_node_public_ip" {
  description = "Master Node public IP"
  value       = aws_instance.master_node.public_ip
}
