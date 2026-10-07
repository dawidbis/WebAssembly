# Bootstrap: zasoby potrzebne, zanim zadziała właściwa infrastruktura (bucket na stan Terraform,
# budżety). Stan tego katalogu jest LOKALNY (terraform.tfstate, poza repo) – uruchamiany raz.

terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }
}

provider "aws" {
  region  = var.region
  profile = var.profile

  default_tags {
    tags = {
      Project   = var.project
      ManagedBy = "terraform"
      Stack     = "bootstrap"
    }
  }
}
