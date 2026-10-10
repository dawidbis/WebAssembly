# Game-server: jedna instancja EC2 (Graviton) z wieloma pokojami w jednym procesie (docs/adr/0001).
# Dostępna wyłącznie przez CloudFront (/ws*): Security Group wpuszcza tylko adresy CloudFront,
# a serwer wymaga nagłówka X-Origin-Verify. Bez SSH – dostęp i wdrożenia przez SSM.

terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
    }
    random = {
      source = "hashicorp/random"
    }
  }
}

data "aws_region" "current" {}
data "aws_caller_identity" "current" {}

locals {
  binary_key       = "game-server/game-server"
  secret_param     = "/${replace(var.name, "-", "/")}/origin-verify-secret"
  ticket_key_param = "/${replace(var.name, "-", "/")}/ticket-public-key"
  log_group        = "/${replace(var.name, "-", "/")}/game-server"
}

# --- Artefakty (binarki do wdrożenia) ---

resource "aws_s3_bucket" "artifacts" {
  bucket        = "${var.name}-artifacts-${data.aws_caller_identity.current.account_id}"
  force_destroy = true
}

resource "aws_s3_bucket_public_access_block" "artifacts" {
  bucket                  = aws_s3_bucket.artifacts.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "artifacts" {
  bucket = aws_s3_bucket.artifacts.id
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "artifacts" {
  bucket = aws_s3_bucket.artifacts.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

# Binarki z poprzednich wdrożeń (game-server/<commit>) – do wycofania zmiany; po 30 dniach znikają.
resource "aws_s3_bucket_lifecycle_configuration" "artifacts" {
  bucket = aws_s3_bucket.artifacts.id
  rule {
    id     = "expire-old-builds"
    status = "Enabled"
    filter {
      prefix = "game-server/builds/"
    }
    expiration {
      days = 30
    }
  }
}

# --- Sekret originu (CloudFront ↔ serwer) ---

resource "random_password" "origin_verify" {
  length  = 40
  special = false
}

resource "aws_ssm_parameter" "origin_verify" {
  name  = local.secret_param
  type  = "SecureString"
  value = random_password.origin_verify.result
}

# --- Logi ---

resource "aws_cloudwatch_log_group" "server" {
  name              = local.log_group
  retention_in_days = var.log_retention_days
}

# --- IAM ---

data "aws_iam_policy_document" "assume_ec2" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "server" {
  name               = "${var.name}-game-server"
  assume_role_policy = data.aws_iam_policy_document.assume_ec2.json
}

resource "aws_iam_role_policy_attachment" "ssm" {
  role       = aws_iam_role.server.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_role_policy_attachment" "cloudwatch_agent" {
  role       = aws_iam_role.server.name
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
}

data "aws_iam_policy_document" "server" {
  statement {
    sid       = "ReadBinary"
    actions   = ["s3:GetObject"]
    resources = ["${aws_s3_bucket.artifacts.arn}/game-server/*"]
  }
  statement {
    sid     = "ReadParams"
    actions = ["ssm:GetParameter"]
    resources = [
      aws_ssm_parameter.origin_verify.arn,
      "arn:aws:ssm:${data.aws_region.current.region}:${data.aws_caller_identity.current.account_id}:parameter${local.ticket_key_param}",
    ]
  }
}

resource "aws_iam_role_policy" "server" {
  name   = "game-server"
  role   = aws_iam_role.server.id
  policy = data.aws_iam_policy_document.server.json
}

resource "aws_iam_instance_profile" "server" {
  name = "${var.name}-game-server"
  role = aws_iam_role.server.name
}

# --- Sieć ---

data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
  filter {
    name   = "default-for-az"
    values = ["true"]
  }
}

data "aws_ec2_managed_prefix_list" "cloudfront" {
  name = "com.amazonaws.global.cloudfront.origin-facing"
}

resource "aws_security_group" "server" {
  name        = "${var.name}-game-server"
  description = "Game server: only CloudFront origin-facing addresses"
  vpc_id      = data.aws_vpc.default.id
}

resource "aws_vpc_security_group_ingress_rule" "cloudfront" {
  security_group_id = aws_security_group.server.id
  description       = "WebSocket from CloudFront"
  ip_protocol       = "tcp"
  from_port         = var.port
  to_port           = var.port
  prefix_list_id    = data.aws_ec2_managed_prefix_list.cloudfront.id
}

resource "aws_vpc_security_group_egress_rule" "all" {
  security_group_id = aws_security_group.server.id
  description       = "S3, SSM, CloudWatch, package repos"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}

# --- Instancja ---

data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-arm64"
}

resource "aws_instance" "server" {
  ami                    = data.aws_ssm_parameter.al2023_arm64.value
  instance_type          = var.instance_type
  subnet_id              = sort(data.aws_subnets.default.ids)[0]
  vpc_security_group_ids = [aws_security_group.server.id]
  iam_instance_profile   = aws_iam_instance_profile.server.name

  associate_public_ip_address = true

  user_data = templatefile("${path.module}/user-data.sh.tftpl", {
    region           = data.aws_region.current.region
    port             = var.port
    room_idle_secs   = var.room_idle_secs
    secret_param     = local.secret_param
    ticket_key_param = local.ticket_key_param
    log_group        = local.log_group
    bucket           = aws_s3_bucket.artifacts.bucket
    binary_key       = local.binary_key
  })
  user_data_replace_on_change = true

  # Bez „unlimited”: przy długim obciążeniu CPU instancja zwalnia zamiast naliczać opłaty.
  credit_specification {
    cpu_credits = "standard"
  }

  metadata_options {
    http_tokens   = "required" # IMDSv2
    http_endpoint = "enabled"
  }

  root_block_device {
    volume_type = "gp3"
    volume_size = 8
    encrypted   = true
  }

  tags = {
    Name = "${var.name}-game-server"
  }

  lifecycle {
    # Nowsze AMI nie wymienia instancji przy każdym apply – świadomie przez `terraform apply -replace`.
    ignore_changes = [ami]
  }
}

# Awaria sprzętu po stronie AWS → automatyczne przeniesienie instancji (ten sam adres, darmowy alarm).
resource "aws_cloudwatch_metric_alarm" "recover" {
  alarm_name          = "${var.name}-game-server-recover"
  alarm_description   = "Recover game server on system status check failure"
  namespace           = "AWS/EC2"
  metric_name         = "StatusCheckFailed_System"
  statistic           = "Maximum"
  period              = 60
  evaluation_periods  = 2
  threshold           = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  dimensions = {
    InstanceId = aws_instance.server.id
  }
  alarm_actions = ["arn:aws:automate:${data.aws_region.current.region}:ec2:recover"]
}
