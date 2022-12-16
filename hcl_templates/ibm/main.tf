# Reference terraform.tfvars variables
variable "public_rsa_key_path" {}

variable "cloud_zone" {}
variable "resource_group_id" {}
variable "resources_base_name" {}
variable "resources_tags" {}

variable "entrypoint_ami" {}
variable "entrypoint_instance_type" {}

variable "worker_ami" {}
variable "worker_count" {}
variable "worker_instance_type" {}

variable "block_storage_iops" {}
variable "block_storage_capacity" {}


# Use this block to specify variables that you want to use multiple times throughout this configuration file
locals {
  BASENAME = var.resources_base_name
  ZONE     = var.cloud_zone
  TAGS     = var.resources_tags
}


# Create a ssh keypair which will be used to provision code onto the system
# and also access the VM for debug if needed.
resource "tls_private_key" "build_key" {
  algorithm = "RSA"
  rsa_bits  = "4096"
}
resource "ibm_is_ssh_key" "build_key" {
  name           = "${local.BASENAME}-ssh-build-key"
  public_key     = tls_private_key.build_key.public_key_openssh
  resource_group = var.resource_group_id
  tags           = local.TAGS
}
# Create an IBM Cloud infrastructure SSH key
resource "ibm_is_ssh_key" "my_ssh_key" {
  name           = "${local.BASENAME}-ssh-public-key"
  public_key     = var.public_rsa_key_path
  resource_group = var.resource_group_id
  tags           = local.TAGS
}

# Create the VPC
# https://cloud.ibm.com/docs/ibm-cloud-provider-for-terraform?topic=ibm-cloud-provider-for-terraform-vpc-gen2-resources#provider-vps
resource "ibm_is_vpc" "vpc" {
  name                        = "${local.BASENAME}-vpc"
  classic_access              = false
  address_prefix_management   = "auto"
  default_network_acl_name    = "${local.BASENAME}-vpc-default-access-control-list"
  default_security_group_name = "${local.BASENAME}-vpc-default-security-group"
  default_routing_table_name  = "${local.BASENAME}-vpc-default-routing-table"
  resource_group              = var.resource_group_id
  tags                        = local.TAGS
}

# Create a custom VPC Security Group
resource "ibm_is_security_group" "my_sec_group" {
  name           = "${local.BASENAME}-vpc-custom-security-group"
  vpc            = ibm_is_vpc.vpc.id
  resource_group = var.resource_group_id
  tags           = local.TAGS
  depends_on = [
    ibm_is_vpc.vpc,
  ]
}

# Create custom SG rule allowing all incoming network traffic on port 22
resource "ibm_is_security_group_rule" "my_sec_rule" {
  group     = ibm_is_security_group.my_sec_group.id
  direction = "inbound"
  remote    = "0.0.0.0/0"
  tcp {
    port_min = 22
    port_max = 22
  }
  depends_on = [
    ibm_is_security_group.my_sec_group,
  ]
}

# Create a VPC Public Gateway
resource "ibm_is_public_gateway" "my_gateway" {
  name           = "${local.BASENAME}-gateway-${local.ZONE}"
  vpc            = ibm_is_vpc.vpc.id
  zone           = local.ZONE
  resource_group = var.resource_group_id
  tags           = local.TAGS
}

# Create a VPC subnet
# https://cloud.ibm.com/docs/ibm-cloud-provider-for-terraform?topic=ibm-cloud-provider-for-terraform-vpc-gen2-resources#subnet
resource "ibm_is_subnet" "my_subnet" {
  name                     = "${local.BASENAME}-subnet-${local.ZONE}"
  vpc                      = ibm_is_vpc.vpc.id
  network_acl              = ibm_is_vpc.vpc.default_network_acl
  routing_table            = ibm_is_vpc.vpc.default_routing_table
  public_gateway           = ibm_is_public_gateway.my_gateway.id
  total_ipv4_address_count = 512
  zone                     = local.ZONE
  resource_group           = var.resource_group_id
  tags                     = local.TAGS
  depends_on = [
    ibm_is_vpc.vpc,
  ]
}

# Create a Block Storage Volume for the NFS
# https://cloud.ibm.com/docs/vpc?topic=vpc-block-storage-profiles#tiers
resource "ibm_is_volume" "my_block_storage" {
  #count          = var.instance_count
  name           = "${local.BASENAME}-nfss-block-storage"
  profile        = "custom"
  iops           = var.block_storage_iops
  capacity       = var.block_storage_capacity
  zone           = local.ZONE
  resource_group = var.resource_group_id
  tags           = local.TAGS
}

# Create a NFS Server VSI for VPC
resource "ibm_is_instance" "my_nfs_server" {
  name    = "${local.BASENAME}-nfss"
  vpc     = ibm_is_vpc.vpc.id
  keys    = [ibm_is_ssh_key.my_ssh_key.id, ibm_is_ssh_key.build_key.id]
  image   = var.entrypoint_ami
  profile = var.entrypoint_instance_type
  boot_volume {
    name = "${local.BASENAME}-nfss-boot-volume"
  }
  volumes = [ibm_is_volume.my_block_storage.id]
  primary_network_interface {
    name            = "${local.BASENAME}-nfss-primary-network-interface"
    subnet          = ibm_is_subnet.my_subnet.id
    security_groups = [ibm_is_security_group.my_sec_group.id, ibm_is_vpc.vpc.default_security_group]
  }
  zone           = local.ZONE
  resource_group = var.resource_group_id
  tags           = local.TAGS
  depends_on = [
    ibm_is_vpc.vpc, ibm_is_subnet.my_subnet, ibm_is_security_group.my_sec_group, ibm_is_ssh_key.my_ssh_key
  ]
}

# Create a floating ip for the NFS Server
resource "ibm_is_floating_ip" "entrypoint_fip" {
  name           = "${local.BASENAME}-entrypoint_fip"
  target         = ibm_is_instance.my_nfs_server.primary_network_interface[0].id
  resource_group = var.resource_group_id
  tags           = local.TAGS
  depends_on     = [ibm_is_instance.my_nfs_server]
}

# Create multiple Worker Node VSIs for VPC
resource "ibm_is_instance" "my_vsi" {
  count   = var.worker_count
  name    = "${local.BASENAME}-worker-${count.index}"
  vpc     = ibm_is_vpc.vpc.id
  keys    = [ibm_is_ssh_key.my_ssh_key.id, ibm_is_ssh_key.build_key.id]
  image   = var.worker_ami
  profile = var.worker_instance_type
  boot_volume {
    name = "${local.BASENAME}-worker-${count.index}-boot-volume"
  }
  primary_network_interface {
    name            = "${local.BASENAME}-worker-${count.index}-primary-network-interface"
    subnet          = ibm_is_subnet.my_subnet.id
    security_groups = [ibm_is_security_group.my_sec_group.id, ibm_is_vpc.vpc.default_security_group]
  }
  zone           = local.ZONE
  resource_group = var.resource_group_id
  tags           = local.TAGS
  depends_on = [
    ibm_is_vpc.vpc, ibm_is_subnet.my_subnet, ibm_is_security_group.my_sec_group, ibm_is_ssh_key.my_ssh_key
  ]
}

# Create and attach a floating IP for the Worker Nodes
resource "ibm_is_floating_ip" "my_floating_ip" {
  count          = var.worker_count
  name           = "${local.BASENAME}-worker-${count.index}-floating-ip"
  target         = ibm_is_instance.my_vsi[count.index].primary_network_interface[0].id
  resource_group = var.resource_group_id
  tags           = local.TAGS
  depends_on     = [ibm_is_instance.my_vsi]
}

resource "null_resource" "config-nfs-server" {
  connection {
    type        = "ssh"
    host        = ibm_is_floating_ip.entrypoint_fip.address
    user        = "root"
    private_key = tls_private_key.build_key.private_key_pem
  }
  provisioner "file" {
    source      = "nfs-server-setup.sh"
    destination = "/tmp/nfs-server-setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs-server-setup.sh",
      "/tmp/nfs-server-setup.sh"
    ]
  }
  depends_on = [ibm_is_floating_ip.entrypoint_fip]
}

resource "null_resource" "config-nfs-clients" {
  count = var.worker_count
  connection {
    type        = "ssh"
    host        = ibm_is_floating_ip.my_floating_ip[count.index].address
    user        = "root"
    private_key = tls_private_key.build_key.private_key_pem
  }
  provisioner "file" {
    source      = "nfs-client-setup.sh"
    destination = "/tmp/nfs-client-setup.sh"
  }
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/nfs-client-setup.sh",
      "/tmp/nfs-client-setup.sh ${ibm_is_instance.my_nfs_server.primary_network_interface[0].primary_ipv4_address}"
    ]
  }
  depends_on = [ibm_is_floating_ip.my_floating_ip, null_resource.config-nfs-server]
}