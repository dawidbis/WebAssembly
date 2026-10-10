# Meta-serwer (lobby): API Gateway HTTP API → Lambda w Ruście → DynamoDB (docs/adr/0002).
# Klucz prywatny biletów podpisuje Lambda; publiczny czyta game-server (docs/adr/0004).

terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
    }
    tls = {
      source = "hashicorp/tls"
    }
    archive = {
      source = "hashicorp/archive"
    }
  }
}

data "aws_region" "current" {}
data "aws_caller_identity" "current" {}

locals {
  param_prefix      = "/${replace(var.name, "-", "/")}"
  private_key_param = "${local.param_prefix}/ticket-private-key"
  public_key_param  = "${local.param_prefix}/ticket-public-key"
  function_name     = "${var.name}-meta"
}

# --- Tabela pokoi ---

resource "aws_dynamodb_table" "rooms" {
  name         = "${var.name}-rooms"
  billing_mode = "PROVISIONED" # 5/5 + indeks 5/5 mieści się w always-free (25/25)
  hash_key     = "pk"
  range_key    = "sk"

  read_capacity  = 5
  write_capacity = 5

  attribute {
    name = "pk"
    type = "S"
  }
  attribute {
    name = "sk"
    type = "S"
  }
  attribute {
    name = "status"
    type = "S"
  }
  attribute {
    name = "createdAt"
    type = "N"
  }

  global_secondary_index {
    name            = "byStatus"
    projection_type = "ALL"
    key_schema {
      attribute_name = "status"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "createdAt"
      key_type       = "RANGE"
    }
    read_capacity  = 5
    write_capacity = 5
  }

  ttl {
    attribute_name = "ttl"
    enabled        = true
  }

  point_in_time_recovery {
    enabled = true
  }
}

# --- Klucze biletów (Ed25519) ---
# Klucz prywatny jest w stanie Terraform (prywatny, szyfrowany bucket z wersjonowaniem) – świadomie,
# jak sekret originu; rotacja: `terraform apply -replace=module.meta.tls_private_key.tickets`.

resource "tls_private_key" "tickets" {
  algorithm = "ED25519"
}

resource "aws_ssm_parameter" "ticket_private_key" {
  name  = local.private_key_param
  type  = "SecureString"
  value = tls_private_key.tickets.private_key_pem_pkcs8 # PKCS#8 – format oczekiwany przez game_ticket::Signer
}

# Gdy ten parametr istnieje, game-server po restarcie wymaga biletów (start.sh).
resource "aws_ssm_parameter" "ticket_public_key" {
  name  = local.public_key_param
  type  = "String"
  value = tls_private_key.tickets.public_key_pem
}

# --- Lambda ---

data "archive_file" "meta" {
  type        = "zip"
  source_file = var.binary_path
  output_path = "${path.root}/.build/meta.zip"
}

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/aws/lambda/${local.function_name}"
  retention_in_days = var.log_retention_days
}

data "aws_iam_policy_document" "assume_lambda" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "lambda" {
  name               = local.function_name
  assume_role_policy = data.aws_iam_policy_document.assume_lambda.json
}

data "aws_iam_policy_document" "lambda" {
  statement {
    sid       = "Logs"
    actions   = ["logs:CreateLogStream", "logs:PutLogEvents"]
    resources = ["${aws_cloudwatch_log_group.lambda.arn}:*"]
  }
  statement {
    sid       = "Rooms"
    actions   = ["dynamodb:GetItem", "dynamodb:PutItem", "dynamodb:Query"]
    resources = [aws_dynamodb_table.rooms.arn, "${aws_dynamodb_table.rooms.arn}/index/byStatus"]
  }
  statement {
    sid       = "TicketKey"
    actions   = ["ssm:GetParameter"]
    resources = [aws_ssm_parameter.ticket_private_key.arn]
  }
}

resource "aws_iam_role_policy" "lambda" {
  name   = "meta"
  role   = aws_iam_role.lambda.id
  policy = data.aws_iam_policy_document.lambda.json
}

resource "aws_lambda_function" "meta" {
  function_name    = local.function_name
  role             = aws_iam_role.lambda.arn
  filename         = data.archive_file.meta.output_path
  source_code_hash = data.archive_file.meta.output_base64sha256
  runtime          = "provided.al2023"
  handler          = "bootstrap"
  architectures    = ["arm64"]
  memory_size      = 256
  timeout          = 10

  environment {
    variables = {
      ROOMS_TABLE      = aws_dynamodb_table.rooms.name
      TICKET_KEY_PARAM = aws_ssm_parameter.ticket_private_key.name
      RUST_LOG         = "info"
    }
  }

  logging_config {
    log_format = "Text"
    log_group  = aws_cloudwatch_log_group.lambda.name
  }

  depends_on = [aws_iam_role_policy.lambda]
}

# --- HTTP API ---

resource "aws_apigatewayv2_api" "meta" {
  name          = local.function_name
  protocol_type = "HTTP"
  description   = "Lobby API (behind CloudFront /api/*)"
}

resource "aws_apigatewayv2_integration" "meta" {
  api_id                 = aws_apigatewayv2_api.meta.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.meta.invoke_arn
  payload_format_version = "2.0"
}

resource "aws_apigatewayv2_route" "meta" {
  for_each = toset(["GET /api/rooms", "POST /api/rooms", "POST /api/rooms/{id}/join"])

  api_id    = aws_apigatewayv2_api.meta.id
  route_key = each.key
  target    = "integrations/${aws_apigatewayv2_integration.meta.id}"
}

resource "aws_cloudwatch_log_group" "api" {
  name              = "/aws/apigateway/${local.function_name}"
  retention_in_days = var.log_retention_days
}

resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.meta.id
  name        = "$default"
  auto_deploy = true

  default_route_settings {
    throttling_rate_limit  = var.throttle_rate
    throttling_burst_limit = var.throttle_burst
  }

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api.arn
    format = jsonencode({
      requestId = "$context.requestId"
      ip        = "$context.identity.sourceIp"
      method    = "$context.httpMethod"
      route     = "$context.routeKey"
      status    = "$context.status"
      latencyMs = "$context.responseLatency"
      error     = "$context.integrationErrorMessage"
    })
  }
}

resource "aws_lambda_permission" "api" {
  statement_id  = "AllowApiGateway"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.meta.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.meta.execution_arn}/*/*"
}
