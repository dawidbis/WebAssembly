output "api_domain" {
  description = "Domena HTTP API (origin CloudFront dla /api/*)."
  value       = replace(aws_apigatewayv2_api.meta.api_endpoint, "https://", "")
}

output "api_origin_secret" {
  value     = random_password.api_origin.result
  sensitive = true
}

output "rooms_table_name" {
  value = aws_dynamodb_table.rooms.name
}

output "rooms_table_arn" {
  value = aws_dynamodb_table.rooms.arn
}

output "function_name" {
  value = aws_lambda_function.meta.function_name
}

output "ticket_public_key_param" {
  value = aws_ssm_parameter.ticket_public_key.name
}
