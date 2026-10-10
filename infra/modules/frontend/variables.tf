variable "name" {
  description = "Prefiks nazw zasobów, np. mapa-prod."
  type        = string
}

variable "aliases" {
  description = "Własne domeny dystrybucji (puste = tylko adres *.cloudfront.net)."
  type        = list(string)
  default     = []
}

variable "certificate_arn" {
  description = "Certyfikat ACM z us-east-1 dla `aliases` (null = certyfikat domyślny CloudFront)."
  type        = string
  default     = null
}

variable "price_class" {
  description = "Zasięg krawędzi CloudFront. PriceClass_100 = Europa i Ameryka Płn. (najtaniej)."
  type        = string
  default     = "PriceClass_100"
}

variable "api_origin_domain" {
  description = "Domena API Gateway HTTP API dla `/api/*` (null = brak zachowania; etap meta)."
  type        = string
  default     = null
}

variable "game_origin" {
  description = "Origin game-servera dla `/ws*` (null = brak zachowania; etap game-server)."
  type = object({
    domain = string
    port   = number
  })
  default = null
}

variable "origin_verify_secret" {
  description = "Wartość nagłówka X-Origin-Verify, który CloudFront dokłada do żądań do game-servera."
  type        = string
  default     = ""
  sensitive   = true
}
