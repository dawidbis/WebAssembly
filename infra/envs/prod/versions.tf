terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # Konfiguracja częściowa – szczegóły w backend.hcl (poza repo; zawartość wypisuje
  # `terraform output backend_hcl` w infra/bootstrap):
  #   terraform init -backend-config=backend.hcl
  backend "s3" {}
}

provider "aws" {
  region  = var.region
  profile = var.profile

  default_tags {
    tags = local.tags
  }
}

# CloudFront przyjmuje certyfikaty ACM tylko z us-east-1.
provider "aws" {
  alias   = "us_east_1"
  region  = "us-east-1"
  profile = var.profile

  default_tags {
    tags = local.tags
  }
}
