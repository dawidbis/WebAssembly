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

variable "log_bucket" {
  description = "Bucket na logi (z infra/bootstrap): logi dostępu S3 i standardowe logi CloudFront."
  type        = string
}

variable "log_bucket_domain" {
  description = "Domena bucketu logów (<bucket>.s3.amazonaws.com) – format wymagany przez logi CloudFront."
  type        = string
}
