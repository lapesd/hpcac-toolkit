# Reference variables from terraform.tfvars
variable "vultr_api_key" {}

# Configure the Cloud Provider
provider "vultr" {
  api_key     = var.vultr_api_key
  rate_limit  = 700
  retry_limit = 3
}