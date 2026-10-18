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
   commit, and one WebTransport handshake to round-trip a message (the
   `webtransport-probe` example in the server crate, run against
   `https://formulanone.sushimafia.com/wt`). It retries for two minutes,
   then fails the job.

Two repository variables feed the job, both set from `tofu output` after an
apply: `AWS_DEPLOY_ROLE_ARN` and `AWS_INSTANCE_ID` (see `infra/README.md`).

## The same deploy from a laptop

For a commit whose image is already in ECR (every commit on main):

```sh
AWS_PROFILE=neconote deploy/deploy.sh <commit-sha>
```

The instance id comes from the OpenTofu state in `infra/`; set `INSTANCE_ID`
to skip that. To smoke-test without deploying (the first run builds the
WebTransport probe):

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

`deploy/release` is gitignored. The laptop override points the server at
the certificate Caddy's local issuer writes, so WebTransport is up at
`https://localhost/wt` too; a browser trusts it only once Caddy's local root
is installed, and falls back to the WebSocket otherwise. The probe trusts
that root when told where it is:

```sh
docker compose -f deploy/release/compose.yaml -f deploy/compose.local.yaml \
  cp caddy:/data/caddy/pki/authorities/local/root.crt /tmp/caddy-root.crt
WEBTRANSPORT_PROBE_CA=/tmp/caddy-root.crt \
  cargo run -p server --example webtransport-probe -- https://localhost/wt
```

## Endpoints

| Path           | Served by | Answer                                        |
| -------------- | --------- | --------------------------------------------- |
| `/`            | Caddy     | the static client                             |
| `/api/health`  | server    | `{"status":"ok"}`                             |
| `/api/version` | server    | `{"commit":"<deployed sha>"}`                 |
| `/api/ws`      | server    | WebSocket carrying the protocol's messages    |
| `/wt`          | server    | WebTransport session on 443 UDP, same messages |

The server listens on 8080 inside the Compose network; Caddy publishes 443
TCP and proxies `/api/*`, the WebSocket fallback included. HTTP/3 is off in
Caddy so 443 UDP is the server's: it terminates QUIC itself for WebTransport
(ADR 0011) and the Compose file publishes it. The server exits promptly on
SIGTERM, closing open sessions, so a `docker compose restart server`
completes well inside Docker's ten-second grace period.

## The certificate and its renewal

WebTransport uses the same Let's Encrypt certificate Caddy obtains. Caddy
keeps it in the `caddy_data` volume at
`/data/caddy/certificates/acme-v02.api.letsencrypt.org-directory/<site>/<site>.{crt,key}`;
the Compose file mounts that volume read-only into the server and names the
two files in `TLS_CERT` and `TLS_KEY`. Caddy writes the key readable by root
only, so the server container runs as root with every capability dropped
and a read-only filesystem.

Renewal needs no rebuild and no restart. Caddy renews by rewriting the same
two files; the server re-reads them once a minute and, when their bytes
change, reloads the TLS configuration of its running QUIC endpoint
(`crates/server/src/webtransport.rs`), logging `webtransport reloaded
certificate`. Sessions already open keep their handshake; new ones get the
renewed certificate. On a first deploy, when Caddy has not obtained a
certificate yet, the server serves health and the WebSocket at once and
starts WebTransport as soon as the files appear.
