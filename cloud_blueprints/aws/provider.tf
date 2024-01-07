variable "region" {}

provider "aws" {
  region      = var.region
  profile     = "default"
  max_retries = 10
}
