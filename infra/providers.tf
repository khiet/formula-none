provider "aws" {
  region  = var.region
  profile = var.aws_profile

  default_tags {
    tags = {
      Project = local.name
    }
  }
}

# The hosted zone for sushimafia.com may live in another account (ADR 0015);
# this alias is the one that writes the A record.
provider "aws" {
  alias   = "dns"
  region  = var.region
  profile = coalesce(var.dns_aws_profile, var.aws_profile)
}
