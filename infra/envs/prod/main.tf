locals {
  name = "${var.project}-${var.environment}"

  tags = {
    Project     = var.project
    Environment = var.environment
    ManagedBy   = "terraform"
  }

  custom_domain = var.domain_name != null
}

data "aws_caller_identity" "current" {}

# Bucket na logi tworzy infra/bootstrap (wspólny dla stanu i środowisk).
data "aws_s3_bucket" "logs" {
  bucket = "${var.project}-logs-${data.aws_caller_identity.current.account_id}"
}

# Własna domena – opcjonalna; bez niej gra działa pod adresem *.cloudfront.net.
module "dns" {
  source = "../../modules/dns"
  count  = local.custom_domain ? 1 : 0

  providers = {
    aws           = aws
    aws.us_east_1 = aws.us_east_1
  }

  domain_name = var.domain_name
  create_zone = var.create_zone
}

module "meta" {
  source = "../../modules/meta"

  name = local.name
  # Buduje tools/deploy/meta.mjs (cargo zigbuild) – przed `terraform plan/apply`.
  binary_path = "${path.root}/../../../target/aarch64-unknown-linux-musl/release/bootstrap"
}

module "game_server" {
  source = "../../modules/game_server"

  name             = local.name
  instance_type    = var.game_server_instance_type
  log_bucket       = data.aws_s3_bucket.logs.id
  rooms_table_name = module.meta.rooms_table_name
  rooms_table_arn  = module.meta.rooms_table_arn
}

module "frontend" {
  source = "../../modules/frontend"

  name            = local.name
  aliases         = local.custom_domain ? [var.domain_name] : []
  certificate_arn = local.custom_domain ? module.dns[0].certificate_arn : null

  log_bucket        = data.aws_s3_bucket.logs.id
  log_bucket_domain = data.aws_s3_bucket.logs.bucket_domain_name

  game_origin          = { domain = module.game_server.origin_domain, port = module.game_server.port }
  origin_verify_secret = module.game_server.origin_verify_secret
  api_origin_domain    = module.meta.api_domain
}

resource "aws_route53_record" "alias" {
  for_each = local.custom_domain ? toset(["A", "AAAA"]) : toset([])

  zone_id = module.dns[0].zone_id
  name    = var.domain_name
  type    = each.key

  alias {
    name                   = module.frontend.domain_name
    zone_id                = module.frontend.hosted_zone_id
    evaluate_target_health = false
  }
}
