aws_secret_key = ""
aws_access_key = ""

region             = "us-east-1"
availability_zones = ["us-east-1a"]

private_rsa_key_path = "./keys/id_rsa"
public_rsa_key_path  = "./keys/id_rsa.pub"
public_key_name      = "vnderlev@archlinux"

entrypoint_ami           = "ami-08e4e35cccc6189f4"
entrypoint_instance_type = "t2.micro"

worker_ami           = "ami-08e4e35cccc6189f4"
worker_instance_type = "t2.micro"
worker_count         = 8
worker_spot_price    = 0.0035
