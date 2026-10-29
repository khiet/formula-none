#!/usr/bin/env bash
# Open Grafana: forward a local port to the Grafana on the instance through
# SSM Session Manager (ADR 0015, ADR 0018), the only way in since nothing
# but 443 is open. Browse http://localhost:3000 while the session runs;
# Ctrl-C ends it.
#
# Usage: deploy/grafana.sh
# Env:   INSTANCE_ID         target instance (default: tofu output instance_id)
#        GRAFANA_LOCAL_PORT  local end of the forward (default: 3000)
#        AWS_PROFILE, AWS_REGION as for the AWS CLI; region defaults to eu-west-2
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
export AWS_REGION=${AWS_REGION:-eu-west-2}
instance_id=${INSTANCE_ID:-$(tofu -chdir="$here/../infra" output -raw instance_id)}
local_port=${GRAFANA_LOCAL_PORT:-3000}

echo "Grafana is at http://localhost:$local_port while this session runs; Ctrl-C ends it" >&2
exec aws ssm start-session \
  --target "$instance_id" \
  --document-name AWS-StartPortForwardingSession \
  --parameters "{\"portNumber\":[\"3000\"],\"localPortNumber\":[\"$local_port\"]}"
