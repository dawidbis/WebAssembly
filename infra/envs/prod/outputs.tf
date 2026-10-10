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

# --- game-server (tools/deploy/game-server.mjs) ---

output "game_server_instance_id" {
  value = module.game_server.instance_id
}

output "artifacts_bucket" {
  value = module.game_server.artifacts_bucket
}

output "game_server_binary_key" {
  value = module.game_server.binary_key
}

output "game_server_log_group" {
  value = module.game_server.log_group
}

output "region" {
  value = var.region
}

# --- lobby (meta) ---

output "meta_function_name" {
  value = module.meta.function_name
}

output "rooms_table" {
  value = module.meta.rooms_table_name
}
