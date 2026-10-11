# Monitoring: alarmy CloudWatch z powiadomieniem e-mail (SNS). Wszystko w darmowym limicie
# (10 alarmów – razem z alarmem `recover` modułu game_server jest ich 9; SNS e-mail – 1000/mies.).
#
# Po pierwszym `apply` AWS wysyła na adres e-mail prośbę o potwierdzenie subskrypcji (dwie – region
# główny i us-east-1, bo alarm CloudFront musi być w us-east-1); bez kliknięcia „Confirm” nie ma powiadomień.

terraform {
  required_providers {
    aws = {
      source                = "hashicorp/aws"
      configuration_aliases = [aws.us_east_1]
    }
  }
}

# --- Powiadomienia ---

resource "aws_sns_topic" "alerts" {
  name = "${var.name}-alerts"
}

resource "aws_sns_topic_subscription" "email" {
  topic_arn = aws_sns_topic.alerts.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

# Metryki CloudFront są tylko w us-east-1, a alarm może powiadamiać tylko temat z tego samego regionu.
resource "aws_sns_topic" "alerts_us_east_1" {
  provider = aws.us_east_1
  name     = "${var.name}-alerts"
}

resource "aws_sns_topic_subscription" "email_us_east_1" {
  provider  = aws.us_east_1
  topic_arn = aws_sns_topic.alerts_us_east_1.arn
  protocol  = "email"
  endpoint  = var.alert_email
}

locals {
  actions = [aws_sns_topic.alerts.arn]
}

# --- Lobby: Lambda, HTTP API, DynamoDB ---

resource "aws_cloudwatch_metric_alarm" "lambda_errors" {
  alarm_name          = "${var.name}-lobby-errors"
  alarm_description   = "Lambda lobby zwraca błędy (logi: zadanie VS Code „AWS: logi lobby”)."
  namespace           = "AWS/Lambda"
  metric_name         = "Errors"
  dimensions          = { FunctionName = var.function_name }
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  threshold           = var.lambda_errors_threshold
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"
  alarm_actions       = local.actions
  ok_actions          = local.actions
}

resource "aws_cloudwatch_metric_alarm" "lambda_throttles" {
  alarm_name          = "${var.name}-lobby-throttles"
  alarm_description   = "Lambda lobby jest dławiona (limit współbieżności konta)."
  namespace           = "AWS/Lambda"
  metric_name         = "Throttles"
  dimensions          = { FunctionName = var.function_name }
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"
  alarm_actions       = local.actions
  ok_actions          = local.actions
}

# 5xx z API Gateway = błąd Lambdy albo integracji (429 z limitów na IP to 4xx – nie alarmują).
resource "aws_cloudwatch_metric_alarm" "api_5xx" {
  alarm_name          = "${var.name}-api-5xx"
  alarm_description   = "HTTP API lobby zwraca 5xx."
  namespace           = "AWS/ApiGateway"
  metric_name         = "5xx"
  dimensions          = { ApiId = var.api_id }
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  threshold           = var.api_5xx_threshold
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"
  alarm_actions       = local.actions
  ok_actions          = local.actions
}

# Tabela ma provisioned capacity (darmowe ≤ 25 RCU/WCU) – dławienie = za mało pojemności.
resource "aws_cloudwatch_metric_alarm" "dynamodb_throttles" {
  alarm_name          = "${var.name}-rooms-throttles"
  alarm_description   = "Tabela pokoi DynamoDB dławi odczyty lub zapisy (za mało RCU/WCU)."
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  threshold           = 1
  treat_missing_data  = "notBreaching"
  alarm_actions       = local.actions
  ok_actions          = local.actions

  metric_query {
    id          = "throttles"
    expression  = "reads + writes"
    label       = "Odczyty i zapisy zdławione"
    return_data = true
  }
  metric_query {
    id = "reads"
    metric {
      namespace   = "AWS/DynamoDB"
      metric_name = "ReadThrottleEvents"
      dimensions  = { TableName = var.table_name }
      stat        = "Sum"
      period      = 300
    }
  }
  metric_query {
    id = "writes"
    metric {
      namespace   = "AWS/DynamoDB"
      metric_name = "WriteThrottleEvents"
      dimensions  = { TableName = var.table_name }
      stat        = "Sum"
      period      = 300
    }
  }
}

# --- Game-server (EC2) ---

# Awaria systemu operacyjnego / procesu na instancji (awarię sprzętu naprawia alarm `recover`).
resource "aws_cloudwatch_metric_alarm" "ec2_status" {
  alarm_name          = "${var.name}-game-server-status"
  alarm_description   = "Instancja game-servera nie przechodzi testów stanu (StatusCheckFailed)."
  namespace           = "AWS/EC2"
  metric_name         = "StatusCheckFailed"
  dimensions          = { InstanceId = var.instance_id }
  statistic           = "Maximum"
  period              = 60
  evaluation_periods  = 3
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  # Brak danych = zatrzymana instancja (świadomie) – bez alarmu.
  treat_missing_data = "notBreaching"
  alarm_actions      = local.actions
  ok_actions         = local.actions
}

# t4g w trybie `standard`: po wyczerpaniu kredytów CPU instancja zwalnia do poziomu bazowego
# (tury zaczną się opóźniać) – ostrzeżenie, zanim to nastąpi.
resource "aws_cloudwatch_metric_alarm" "ec2_cpu_credits" {
  alarm_name          = "${var.name}-game-server-cpu-credits"
  alarm_description   = "Kończą się kredyty CPU game-servera – za chwilę zwolni (rozważ większą instancję)."
  namespace           = "AWS/EC2"
  metric_name         = "CPUCreditBalance"
  dimensions          = { InstanceId = var.instance_id }
  statistic           = "Minimum"
  period              = 300
  evaluation_periods  = 2
  threshold           = var.cpu_credits_threshold
  comparison_operator = "LessThanThreshold"
  treat_missing_data  = "notBreaching"
  alarm_actions       = local.actions
  ok_actions          = local.actions
}

# --- CloudFront (cała gra: frontend, /api/*, /ws*) ---

resource "aws_cloudwatch_metric_alarm" "cloudfront_5xx" {
  provider            = aws.us_east_1
  alarm_name          = "${var.name}-cloudfront-5xx"
  alarm_description   = "Odsetek odpowiedzi 5xx w CloudFront (np. game-server nie odpowiada na /ws, błąd originu)."
  namespace           = "AWS/CloudFront"
  metric_name         = "5xxErrorRate"
  dimensions          = { DistributionId = var.distribution_id, Region = "Global" }
  statistic           = "Average"
  period              = 300
  evaluation_periods  = 2
  threshold           = var.cloudfront_5xx_rate
  comparison_operator = "GreaterThanOrEqualToThreshold"
  treat_missing_data  = "notBreaching"
  alarm_actions       = [aws_sns_topic.alerts_us_east_1.arn]
  ok_actions          = [aws_sns_topic.alerts_us_east_1.arn]
}
