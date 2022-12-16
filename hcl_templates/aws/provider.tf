# Reference variables from terraform.tfvars
variable "aws_access_key" {}
variable "aws_secret_key" {}
variable "region" {}

# Configure the Cloud Provider
provider "aws" {
  region      = var.region
  profile     = "default"
  access_key  = var.aws_access_key
  secret_key  = var.aws_secret_key
  max_retries = 5
}