variable "name" {
  description = "Prefiks nazw zasobów, np. mapa-prod."
  type        = string
}

variable "instance_type" {
  description = "Typ instancji (Graviton). t4g.micro: 2 vCPU, 1 GB RAM."
  type        = string
  default     = "t4g.micro"
}

variable "port" {
  description = "Port game-servera (HTTP/WebSocket) – dostępny tylko z CloudFront."
  type        = number
  default     = 3000
}

variable "room_idle_secs" {
  description = "Po ilu sekundach pusty pokój jest zamykany."
  type        = number
  default     = 300
}

variable "log_retention_days" {
  description = "Retencja logów w CloudWatch."
  type        = number
  default     = 14
}

variable "log_bucket" {
  description = "Bucket na logi dostępu S3 (z infra/bootstrap)."
  type        = string
}

variable "rooms_table_name" {
  description = "Tabela pokoi lobby (heartbeat game-servera)."
  type        = string
}

variable "rooms_table_arn" {
  type = string
}
