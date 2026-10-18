#!/usr/bin/env bash
# Smoke-test a deploy: the client, health and version must answer at the
# base URL, and version must name the commit that was deployed. Retries
# until SMOKE_TIMEOUT seconds (default 120) have passed, because Compose
# returns before the server listens and Caddy may still be fetching its
# first certificate. Exits non-zero on any miss, which fails the deploy job.
#
# Usage: deploy/smoke.sh <commit-sha> [base-url]
set -euo pipefail

commit=${1:?usage: deploy/smoke.sh <commit-sha> [base-url]}
base_url=${2:-https://formulanone.sushimafia.com}
deadline=$(( $(date +%s) + ${SMOKE_TIMEOUT:-120} ))

# Prints "<status> <content-type>" for a path; "000" when nothing answered.
probe() {
  curl --silent --show-error --max-time 10 --output "$body_file" \
    --write-out '%{http_code} %{content_type}\n' "$base_url$1" || echo "000"
}

body_file=$(mktemp)
trap 'rm -f "$body_file"' EXIT

while :; do
  failure=""
  read -r status content_type < <(probe /api/health)
  [[ $status == 200 ]] || failure="health answered $status"

  if [[ -z $failure ]]; then
    read -r status content_type < <(probe /api/version)
    deployed=$(sed -n 's/.*"commit": *"\([^"]*\)".*/\1/p' "$body_file")
    if [[ $status != 200 ]]; then
      failure="version answered $status"
    elif [[ $deployed != "$commit" ]]; then
      failure="version reports ${deployed:-nothing}, expected $commit"
    fi
  fi

  if [[ -z $failure ]]; then
    read -r status content_type < <(probe /)
    [[ $status == 200 && $content_type == text/html* ]] || failure="client answered $status ${content_type:-}"
  fi

  if [[ -z $failure ]]; then
    echo "smoke ok: $base_url serves commit $commit"
    exit 0
  fi
  if (( $(date +%s) >= deadline )); then
    echo "smoke failed: $failure" >&2
    exit 1
  fi
  echo "waiting: $failure"
  sleep 3
done
