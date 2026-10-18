# ADR 0017: Cost ceiling

## Status

Accepted (2026-10-18)

## Context

The game should stay up indefinitely for friends, paid for by one person.

## Decision

- The whole environment costs under USD 25 a month.
- To stay under it: one t4g.small, one Elastic IP, ECR, CloudWatch Logs with
  14-day retention. No NAT gateway, no load balancer, no RDS, no CDN at v1.

## Consequences

- Any infrastructure change is checked against the ceiling before it is
  applied.
- Multi-region, ECS Fargate and persistence of any kind are deferred to v1.1
  or later.
- A monthly cost check is part of milestone M5.
