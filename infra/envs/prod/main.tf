locals {
  name = "${var.project}-${var.environment}"

  tags = {
    Project     = var.project
    Environment = var.environment
    ManagedBy   = "terraform"
  }

  custom_domain = var.domain_name != null
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

module "frontend" {
  source = "../../modules/frontend"

  name            = local.name
  aliases         = local.custom_domain ? [var.domain_name] : []
  certificate_arn = local.custom_domain ? module.dns[0].certificate_arn : null

  # Kolejne etapy: api_origin_domain (meta), game_origin + origin_verify_secret (game-server).
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
