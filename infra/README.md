# Infrastructure

OpenTofu for the single-instance environment described in ADR 0015: VPC,
subnet, security group, one t4g.small with an Elastic IP, DNS record, ECR
repository, CloudWatch log group, and the GitHub Actions OIDC role. Applied
by hand from the laptop with the `neconote` profile in `eu-west-2`; nothing
here runs from CI.

Cost ceiling: USD 25 a month (ADR 0017). Check the estimate before applying.
