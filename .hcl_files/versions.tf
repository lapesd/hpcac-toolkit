terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
      version = "~> 4.30.0"
    }
    null = {
      source = "hashicorp/null"
    }
    tls = {
      source = "hashicorp/tls"
    }
  }
}
