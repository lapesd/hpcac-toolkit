variable "aws_access_key" {}
variable "aws_secret_key" {}
variable "region" {}

provider "aws" {
  region      = var.region
  profile     = "default"
  max_retries = 5
  access_key = var.aws_access_key
  secret_key = var.aws_secret_key
}
