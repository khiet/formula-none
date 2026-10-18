# Deploy

What a merge to main ships and how to ship the same thing by hand (ADR
0016). The environment itself is in `infra/`.

## What the pipeline does

The `deploy` job in `.github/workflows/ci.yml` runs after every other job
has passed on a push to main:

1. Builds the image from the `Dockerfile` for arm64 and pushes it to ECR
   tagged with the commit. The image holds the server binary and, under
   `/release`, this directory's `compose.yaml` and `Caddyfile` plus the
   built client.
2. Runs `deploy/deploy.sh <commit>`: the deploy document (an SSM Run Command
   defined in `infra/deploy.tf`) pulls the image on the instance, copies
   `/release` out of it to `/opt/formula-none/release`, and runs
   `docker compose pull` and `up` from there. The script waits for the
   command, prints its output and fails on anything but success.
3. `deploy/smoke.sh <commit>` then requires `https://formulanone.sushimafia.com`
   to answer the client, `/api/health`, and `/api/version` naming that
   commit. It retries for two minutes, then fails the job.

Two repository variables feed the job, both set from `tofu output` after an
apply: `AWS_DEPLOY_ROLE_ARN` and `AWS_INSTANCE_ID` (see `infra/README.md`).

## The same deploy from a laptop

For a commit whose image is already in ECR (every commit on main):

```sh
AWS_PROFILE=neconote deploy/deploy.sh <commit-sha>
```

The instance id comes from the OpenTofu state in `infra/`; set `INSTANCE_ID`
to skip that. To smoke-test without deploying:

```sh
deploy/smoke.sh <commit-sha>
```

## Building the image by hand

```sh
docker build --platform linux/arm64 --build-arg COMMIT=$(git rev-parse HEAD) -t formula-none:dev .
```

The build runs the Rust and web toolchains inside the image, so Docker is the
only requirement. `COMMIT` is what `/api/version` reports.

## Running the stack locally

Extract the release bundle from a local image, then start Compose with the
laptop overrides (no CloudWatch; Caddy serves `https://localhost` with its
own local CA, so pass `-k` to curl):

```sh
container=$(docker create formula-none:dev)
docker cp "$container:/release" deploy/release && docker rm "$container"
SERVER_IMAGE=formula-none:dev docker compose \
  -f deploy/release/compose.yaml -f deploy/compose.local.yaml up
```

`deploy/release` is gitignored.

## Endpoints

| Path           | Served by | Answer                        |
| -------------- | --------- | ----------------------------- |
| `/`            | Caddy     | the static client             |
| `/api/health`  | server    | `{"status":"ok"}`             |
| `/api/version` | server    | `{"commit":"<deployed sha>"}` |

The server listens on 8080 inside the Compose network only; Caddy publishes
443 TCP and proxies `/api/*`. HTTP/3 is off so 443 UDP stays free for the
server's QUIC listener (ADR 0011). The server exits promptly on SIGTERM, so a
`docker compose restart server` completes well inside Docker's ten-second
grace period.
