variable "project" {
  description = "Nazwa projektu – prefiks nazw zasobów i tag Project."
  type        = string
  default     = "mapa"
}

variable "environment" {
  description = "Nazwa środowiska (prod, później staging)."
  type        = string
  default     = "prod"
}

variable "region" {
  description = "Region główny."
  type        = string
  default     = "eu-central-1"
}

variable "profile" {
  description = "Profil AWS CLI (SSO)."
  type        = string
  default     = "mapa"
}

variable "domain_name" {
  description = "Domena gry (strefa w Route 53), np. gra.example.com albo example.com."
  type        = string
}
