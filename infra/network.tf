resource "aws_vpc" "main" {
  cidr_block           = "10.42.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = { Name = local.name }
}

resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id

  tags = { Name = local.name }
}

# One public subnet; the instance gets a public address at launch so first
# boot can reach the package mirrors before the Elastic IP is attached.
resource "aws_subnet" "public" {
  vpc_id                  = aws_vpc.main.id
  cidr_block              = "10.42.0.0/24"
  availability_zone       = "${var.region}a"
  map_public_ip_on_launch = true

  tags = { Name = "${local.name}-public" }
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }

  tags = { Name = "${local.name}-public" }
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

# Inbound is 443 only: Caddy terminates HTTPS on TCP and the server
# terminates QUIC on UDP (ADR 0015). Port 80 stays closed, so Caddy must
# prove the certificate with TLS-ALPN rather than HTTP-01.
resource "aws_security_group" "instance" {
  name        = local.name
  description = "Game server: HTTPS and QUIC in, everything out"
  vpc_id      = aws_vpc.main.id

  tags = { Name = local.name }
}

resource "aws_vpc_security_group_ingress_rule" "https_tcp" {
  security_group_id = aws_security_group.instance.id
  description       = "HTTPS, including the WebSocket fallback"
  ip_protocol       = "tcp"
  from_port         = 443
  to_port           = 443
  cidr_ipv4         = "0.0.0.0/0"
}

resource "aws_vpc_security_group_ingress_rule" "quic_udp" {
  security_group_id = aws_security_group.instance.id
  description       = "QUIC for WebTransport"
  ip_protocol       = "udp"
  from_port         = 443
  to_port           = 443
  cidr_ipv4         = "0.0.0.0/0"
}

resource "aws_vpc_security_group_egress_rule" "all" {
  security_group_id = aws_security_group.instance.id
  description       = "Image pulls, Lets Encrypt, SSM and CloudWatch endpoints"
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}
