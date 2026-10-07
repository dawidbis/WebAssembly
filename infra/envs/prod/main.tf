locals {
  name = "${var.project}-${var.environment}"

  tags = {
    Project     = var.project
    Environment = var.environment
    ManagedBy   = "terraform"
  }
}

# Moduły dochodzą w kolejnych etapach (plan w infra/README.md):
#   module "dns"         – strefa Route 53 + certyfikat ACM (us-east-1)
#   module "frontend"    – S3 + CloudFront (/*), zachowania /api/* i /ws*
#   module "game_server" – EC2 t4g.micro z game-serverem (/ws*)
#   module "meta"        – Lambda (Rust) + API Gateway HTTP API + DynamoDB (/api/*)
