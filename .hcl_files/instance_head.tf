resource "aws_instance" "head" {
  ami = "ami-08e4e35cccc6189f4"
  instance_type = "t2.nano"
  security_groups = [
    aws_security_group.allow_ssh.id,
    aws_security_group.allow_nfs.id,
    aws_security_group.allow_mpi.id
  ]
  subnet_id = aws_subnet.cluster_subnet.id
  key_name = aws_key_pair.deployer_key.key_name

  root_block_device {
    delete_on_termination = true
    volume_size = 10
  }

  ebs_block_device {
    delete_on_termination = true
    device_name = "/dev/sdh"
    volume_size = 10
  }

  private_ip = "10.0.0.10"
  depends_on = [
    aws_internet_gateway.cluster_ig
  ]
}
