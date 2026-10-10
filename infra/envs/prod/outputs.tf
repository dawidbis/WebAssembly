# Wyjścia czytane przez skrypty wdrożenia (tools/deploy/*.mjs: `terraform output -json`).

output "url" {
  description = "Adres gry."
  value       = "https://${var.domain_name != null ? var.domain_name : module.frontend.domain_name}"
}

output "web_bucket" {
  value = module.frontend.bucket
}

output "distribution_id" {
  value = module.frontend.distribution_id
}

output "name_servers" {
  description = "NS do wpisania u rejestratora (tylko przy create_zone = true)."
  value       = var.domain_name != null ? module.dns[0].name_servers : []
}
