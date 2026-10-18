data "aws_route53_zone" "parent" {
  provider     = aws.dns
  name         = var.zone_name
  private_zone = false
}

resource "aws_route53_record" "game" {
  provider = aws.dns
  zone_id  = data.aws_route53_zone.parent.zone_id
  name     = var.hostname
  type     = "A"
  ttl      = 300
  records  = [aws_eip.server.public_ip]
}
