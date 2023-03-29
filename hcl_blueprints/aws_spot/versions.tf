terraform {
  required_providers {
    null = {
      source  = "hashicorp/null"
      version = "~> 3.2.1"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0.4"
    }
    aws = {
      source  = "hashicorp/aws"
      version = "~> 3.63.0"
    }
  }
}
