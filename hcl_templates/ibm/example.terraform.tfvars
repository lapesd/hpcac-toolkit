ibmcloud_api_key = ""

cloud_zone          = "us-south-3"
resource_group_id   = "38ce055b187b4fc5a8c3efd074dd6b12"
resources_base_name = "julia-mpi"
resources_tags      = ["julia-mpi-demo"]

public_rsa_key_path = "/home/vnderlev/UFSC/spot-cloud-hpc/terraform/aws/keys/id_rsa.pub"

entrypoint_ami           = "r006-13938c0a-89e4-4370-b59b-55cd1402562d"
entrypoint_instance_type = "cx2-2x4"

# to list available OS images run `ibmcloud is images`
# to list available instance profiles run `ibmcloud is instance-profiles`
worker_ami           = "r006-13938c0a-89e4-4370-b59b-55cd1402562d"
worker_instance_type = "bx2d-2x8"
worker_count         = 5

block_storage_iops     = 1000
block_storage_capacity = 10
