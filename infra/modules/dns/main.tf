# Własna domena (opcjonalna): strefa Route 53 i certyfikat ACM w us-east-1 (wymóg CloudFront).
# Rekordy alias na dystrybucję tworzy środowisko (envs/*) – wymagają już istniejącej dystrybucji.

terraform {
  required_providers {
    aws = {
      source                = "hashicorp/aws"
      configuration_aliases = [aws.us_east_1]
    }
  }
}

variable "domain_name" {
  description = "Domena gry, np. example.com albo gra.example.com."
  type        = string
}

variable "create_zone" {
  description = "true = utwórz strefę (domena u innego rejestratora – potem wpisz NS u niego); false = użyj istniejącej (domena kupiona w Route 53)."
  type        = bool
  default     = false
}

resource "aws_route53_zone" "main" {
  count = var.create_zone ? 1 : 0
  name  = var.domain_name
}

data "aws_route53_zone" "existing" {
  count        = var.create_zone ? 0 : 1
  name         = var.domain_name
  private_zone = false
}

locals {
  zone_id = var.create_zone ? aws_route53_zone.main[0].zone_id : data.aws_route53_zone.existing[0].zone_id
}

resource "aws_acm_certificate" "main" {
  provider          = aws.us_east_1
  domain_name       = var.domain_name
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_route53_record" "validation" {
  for_each = {
    for o in aws_acm_certificate.main.domain_validation_options : o.domain_name => o
  }

  zone_id         = local.zone_id
  name            = each.value.resource_record_name
  type            = each.value.resource_record_type
  records         = [each.value.resource_record_value]
  ttl             = 300
  allow_overwrite = true
}

resource "aws_acm_certificate_validation" "main" {
  provider                = aws.us_east_1
  certificate_arn         = aws_acm_certificate.main.arn
  validation_record_fqdns = [for r in aws_route53_record.validation : r.fqdn]
}

output "zone_id" {
  value = local.zone_id
}

output "certificate_arn" {
  description = "Zwalidowany certyfikat (CloudFront poczeka na walidację)."
  value       = aws_acm_certificate_validation.main.certificate_arn
}

output "name_servers" {
  description = "Serwery NS do wpisania u rejestratora (tylko gdy create_zone = true)."
  value       = var.create_zone ? aws_route53_zone.main[0].name_servers : []
}
