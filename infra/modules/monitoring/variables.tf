variable "name" {
  description = "Prefiks nazw zasobów (np. mapa-prod)."
  type        = string
}

variable "alert_email" {
  description = "Adres e-mail na powiadomienia o alarmach (wymaga potwierdzenia subskrypcji)."
  type        = string
}

variable "function_name" {
  description = "Nazwa Lambdy lobby."
  type        = string
}

variable "api_id" {
  description = "ID HTTP API lobby."
  type        = string
}

variable "table_name" {
  description = "Tabela pokoi (DynamoDB)."
  type        = string
}

variable "instance_id" {
  description = "Instancja game-servera."
  type        = string
}

variable "distribution_id" {
  description = "Dystrybucja CloudFront."
  type        = string
}

# --- Progi (stroisz tutaj albo w wywołaniu modułu) ---

variable "lambda_errors_threshold" {
  description = "Tyle błędów Lambdy w 5 minut = alarm."
  type        = number
  default     = 3
}

variable "api_5xx_threshold" {
  description = "Tyle odpowiedzi 5xx z HTTP API w 5 minut = alarm."
  type        = number
  default     = 3
}

variable "cpu_credits_threshold" {
  description = "Poniżej tylu kredytów CPU = alarm (t4g.micro zbiera maks. 288)."
  type        = number
  default     = 30
}

variable "cloudfront_5xx_rate" {
  description = "Odsetek (%) odpowiedzi 5xx w CloudFront przez 10 minut = alarm."
  type        = number
  default     = 5
}
