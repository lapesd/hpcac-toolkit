terraform {
  required_providers {
    vultr = {
      source  = "vultr/vultr"
      version = "~> 2.12.1"
    }
    null = {
      source = "hashicorp/null"
    }
    tls = {
      source = "hashicorp/tls"
    }
  }
}