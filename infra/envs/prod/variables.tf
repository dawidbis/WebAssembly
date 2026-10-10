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
  description = "Własna domena gry, np. example.com (null = tylko adres *.cloudfront.net)."
  type        = string
  default     = null
}

variable "create_zone" {
  description = "true = Terraform tworzy strefę Route 53 (domena u innego rejestratora); false = strefa już istnieje (domena kupiona w Route 53)."
  type        = bool
  default     = false
}
