output "state_bucket" {
  description = "Bucket na stan – wpisz do infra/envs/prod/backend.hcl."
  value       = aws_s3_bucket.state.bucket
}

output "backend_hcl" {
  description = "Gotowa zawartość infra/envs/prod/backend.hcl."
  value       = <<-EOT
    bucket       = "${aws_s3_bucket.state.bucket}"
    key          = "prod/terraform.tfstate"
    region       = "${var.region}"
    profile      = "${var.profile}"
    encrypt      = true
    use_lockfile = true
  EOT
}
