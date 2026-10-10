output "bucket" {
  description = "Bucket z plikami frontendu (cel `aws s3 sync`)."
  value       = aws_s3_bucket.web.bucket
}

output "distribution_id" {
  description = "ID dystrybucji (do inwalidacji)."
  value       = aws_cloudfront_distribution.main.id
}

output "distribution_arn" {
  value = aws_cloudfront_distribution.main.arn
}

output "domain_name" {
  description = "Adres *.cloudfront.net."
  value       = aws_cloudfront_distribution.main.domain_name
}

output "hosted_zone_id" {
  description = "Strefa CloudFront – do rekordów alias w Route 53."
  value       = aws_cloudfront_distribution.main.hosted_zone_id
}
