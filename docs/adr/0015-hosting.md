# ADR 0015: Hosting

## Status

Accepted (2026-10-18)

## Context

One developer, a personal AWS account, friends in the UK and nearby, and a
hard cost ceiling (ADR 0017).

## Decision

- AWS personal account (the neconote profile), region eu-west-2 (London).
- One t4g.small (arm64) with an Elastic IP; security group allowing 443 TCP
  and UDP inbound and nothing else; access by SSM Session Manager with no
  SSH key; Docker and Compose on the instance.
- DNS: `formulanone.sushimafia.com` as an A record to the Elastic IP in the
  hosted zone that holds `sushimafia.com`. If that zone lives in another
  account, the record is added there with a second provider alias.
- TLS: Caddy in Compose terminates HTTPS and serves the static client; the
  Rust server terminates its own QUIC using the same Let's Encrypt
  certificate mounted from Caddy's data volume. One cert, one hostname.
- Compose services: caddy, server, prometheus, grafana. Logs go to
  CloudWatch Logs via the awslogs driver with 14-day retention. Grafana is
  reachable only through an SSM port forward.
- OpenTofu in `infra/` for VPC, subnet, security group, instance, Elastic
  IP, DNS record, ECR repository, CloudWatch log group, and an IAM role with
  an OIDC trust for GitHub Actions scoped to ECR push and one SSM Run
  Command. Applied by hand from the laptop.

## Consequences

- No NAT gateway, load balancer, RDS or CDN at v1.
- A single instance means a deploy or a crash ends races in progress;
  acceptable at friends scale.
- The OpenTofu plan is reviewed by hand; nothing applies from CI.
