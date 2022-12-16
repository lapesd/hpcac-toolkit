# Configure the IBM-Cloud plugin for Terraform
terraform {
  required_providers {
    vultr = {
      source  = "vultr/vultr"
      version = "~> 2.11.0"
    }
    null = {
      source = "hashicorp/null"
    }
    tls = {
      source = "hashicorp/tls"
    }
  }
}