variable "name" {
  description = "Prefiks nazw zasobów, np. mapa-prod."
  type        = string
}

variable "binary_path" {
  description = "Zbudowana binarka Lambdy (`bootstrap`, Linux arm64) – buduje ją tools/deploy/meta.mjs."
  type        = string
}

variable "log_retention_days" {
  type    = number
  default = 14
}

variable "throttle_rate" {
  description = "Średni limit żądań API na sekundę (cały etap)."
  type        = number
  default     = 10
}

variable "throttle_burst" {
  type    = number
  default = 20
}
