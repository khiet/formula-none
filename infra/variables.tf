variable "aws_profile" {
  description = "Named profile for the personal account that hosts the environment."
  type        = string
  default     = "neconote"
}

variable "dns_aws_profile" {
  description = "Named profile for the account holding the hosted zone; null when it is the same account."
  type        = string
  default     = null
}

variable "region" {
  description = "London: closest to the players (ADR 0015)."
  type        = string
  default     = "eu-west-2"
}

variable "zone_name" {
  description = "Existing public hosted zone that receives the A record."
  type        = string
  default     = "sushimafia.com"
}

variable "hostname" {
  description = "Fully qualified name the game is served from; one hostname, one certificate."
  type        = string
  default     = "formulanone.sushimafia.com"
}

variable "github_repository" {
  description = "owner/name of the repository whose main-branch workflows may assume the deploy role."
  type        = string
  default     = "khiet/formula-none"
}

# No default: the repository is public, so the address is passed at apply time
# (infra/README.md) and never committed.
variable "billing_alert_email" {
  description = "Where the monthly budget mails its alerts."
  type        = string
}

variable "instance_type" {
  description = "The one arm64 instance the cost ceiling allows (ADR 0017)."
  type        = string
  default     = "t4g.small"
}

variable "root_volume_gib" {
  description = "Room for the images, Caddy's data volume and Prometheus' retention window."
  type        = number
  default     = 20
}

variable "log_retention_days" {
  description = "How long container logs stay in CloudWatch."
  type        = number
  default     = 14
}

variable "compose_version" {
  description = "Docker Compose plugin release installed on first boot; AL2023 does not package it."
  type        = string
  default     = "v5.6.0"
}
