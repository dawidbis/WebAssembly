variable "project" {
  description = "Nazwa projektu – prefiks nazw zasobów i tag Project."
  type        = string
  default     = "mapa"
}

variable "region" {
  description = "Region główny (Frankfurt – najbliżej Polski)."
  type        = string
  default     = "eu-central-1"
}

variable "profile" {
  description = "Profil AWS CLI (IAM Identity Center / SSO), np. po `aws sso login --profile wieczko`."
  type        = string
  default     = "wieczko"
}

variable "budget_email" {
  description = "Adres e-mail na alarmy budżetowe."
  type        = string
}

variable "budget_limits_usd" {
  description = "Progi miesięcznych budżetów w USD (Free plan: 2 budżety są darmowe)."
  type        = list(number)
  default     = [1, 10]

  validation {
    condition     = length(var.budget_limits_usd) <= 2
    error_message = "Darmowe są tylko 2 budżety."
  }
}

variable "log_retention_days" {
  description = "Po ilu dniach znikają logi dostępu (S3, CloudFront)."
  type        = number
  default     = 30
}
