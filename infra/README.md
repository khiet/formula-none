# Infrastructure

OpenTofu for the single-instance environment described in ADR 0015: VPC,
subnet, security group, one t4g.small with an Elastic IP, DNS record, ECR
repository, CloudWatch log group, the deploy Run Command document, and the
GitHub Actions OIDC role. Applied by hand from the laptop with the `neconote`
profile in `eu-west-2`; nothing here runs from CI.

Cost ceiling: USD 25 a month (ADR 0017). The estimate for what this applies
is in ADR 0015; check it before applying a change.

## Applying

State is local to the laptop and gitignored; there is one environment and one
operator.

```sh
cd infra
tofu init
tofu plan          # review; expect a no-op after the first apply
tofu apply
```

The profile needs rights to create everything here plus `ssm:GetParameter`
on the public AMI parameters and `route53:ListHostedZones`. If the
`sushimafia.com` zone lives in another account, pass its profile:

```sh
tofu apply -var dns_aws_profile=<profile>
```

Inbound port 80 is closed, so Let's Encrypt cannot reach an HTTP-01
challenge; Caddy falls back to TLS-ALPN-01 on 443 by itself.

## Reaching the instance

No key pair exists. Open a shell through Session Manager:

```sh
aws ssm start-session --profile neconote --target "$(tofu output -raw instance_id)"
```

Grafana is reached the same way, with a port forward (ADR 0015).

## Deploying

The deploy workflow assumes `github_actions_role_arn`, pushes to
`ecr_repository_url` tagged with the commit, and runs the
`deploy_document_name` document against `instance_id` with that tag as
`ImageTag`. The document logs in to ECR, pulls the image, copies the release
bundle it carries (Compose file, Caddyfile, built client) to
`/opt/formula-none/release`, and runs `compose pull` and `up` from there.
Nothing here writes the Compose file; it travels inside the image, so the
stack definition always matches the deployed commit.

After an apply, give the workflow the two values it cannot derive:

```sh
gh variable set AWS_DEPLOY_ROLE_ARN --body "$(tofu output -raw github_actions_role_arn)"
gh variable set AWS_INSTANCE_ID --body "$(tofu output -raw instance_id)"
```

The same deploy by hand, including the smoke test, is one command
(`deploy/README.md`):

```sh
AWS_PROFILE=neconote deploy/deploy.sh <commit-sha>
```

## Checks

`tools/repo-checks` parses the configuration and asserts the contract from
issue 5: two inbound rules, no key pair, 14-day retention, the A record, and
the exact permissions of the deploy role. `tofu fmt -check` and
`tofu validate` run locally.
