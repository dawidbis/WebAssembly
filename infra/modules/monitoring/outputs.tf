output "alerts_topic_arn" {
  description = "Temat SNS z powiadomieniami (region główny) – można podpiąć kolejne alarmy."
  value       = aws_sns_topic.alerts.arn
}
