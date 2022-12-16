# Reference variables from terraform.tfvars
variable "ibmcloud_api_key" {}

# Configure the Cloud Provider
provider "ibm" {
  ibmcloud_api_key = var.ibmcloud_api_key
  region           = "us-south"
  profile          = "default"
  max_retries      = 5
}