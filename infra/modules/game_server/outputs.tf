output "origin_domain" {
  description = "Publiczna nazwa DNS instancji – origin CloudFront dla /ws*. Zmienia się po stop/start instancji (wtedy `terraform apply`)."
  value       = aws_instance.server.public_dns
}

output "port" {
  value = var.port
}

output "origin_verify_secret" {
  value     = random_password.origin_verify.result
  sensitive = true
}

output "instance_id" {
  value = aws_instance.server.id
}

output "artifacts_bucket" {
  value = aws_s3_bucket.artifacts.bucket
}

output "binary_key" {
  value = local.binary_key
}

output "log_group" {
  value = aws_cloudwatch_log_group.server.name
}

output "ticket_key_param" {
  description = "Nazwa parametru SSM z kluczem publicznym biletów (tworzy go etap meta)."
  value       = local.ticket_key_param
}
