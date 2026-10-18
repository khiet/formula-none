#!/usr/bin/env bash
# Deploy one commit whose image is already in ECR: run the deploy document
# on the instance through SSM Run Command, wait for it, print its output,
# then smoke-test the site. The merge-to-main job and a laptop run the same
# script; the instance id comes from the environment on CI and from the
# OpenTofu state on a laptop.
#
# Usage: deploy/deploy.sh <commit-sha>
# Env:   INSTANCE_ID       target instance (default: tofu output instance_id)
#        DEPLOY_DOCUMENT   SSM document name (default: formula-none-deploy)
#        AWS_PROFILE, AWS_REGION as for the AWS CLI; region defaults to eu-west-2
set -euo pipefail

commit=${1:?usage: deploy/deploy.sh <commit-sha>}
here=$(cd "$(dirname "$0")" && pwd)
export AWS_REGION=${AWS_REGION:-eu-west-2}
instance_id=${INSTANCE_ID:-$(tofu -chdir="$here/../infra" output -raw instance_id)}
document=${DEPLOY_DOCUMENT:-formula-none-deploy}

command_id=$(aws ssm send-command \
  --document-name "$document" \
  --instance-ids "$instance_id" \
  --parameters "ImageTag=$commit" \
  --query Command.CommandId --output text)
echo "deploying $commit to $instance_id (command $command_id)"

# The invocation appears a moment after the command is accepted, so that
# one error reads as Pending; any other error (wrong instance, missing
# permission) fails at once rather than after the timeout.
error_file=$(mktemp)
trap 'rm -f "$error_file"' EXIT
deadline=$(( $(date +%s) + 600 ))
while :; do
  if ! status=$(aws ssm get-command-invocation \
      --command-id "$command_id" --instance-id "$instance_id" \
      --query Status --output text 2>"$error_file"); then
    grep -q InvocationDoesNotExist "$error_file" || { cat "$error_file" >&2; exit 1; }
    status=Pending
  fi
  case $status in
    Pending | InProgress | Delayed) ;;
    *) break ;;
  esac
  if (( $(date +%s) >= deadline )); then
    echo "deploy still $status after 600 s" >&2
    exit 1
  fi
  sleep 5
done

aws ssm get-command-invocation \
  --command-id "$command_id" --instance-id "$instance_id" \
  --query '[StandardOutputContent, StandardErrorContent]' --output text
if [[ $status != Success ]]; then
  echo "deploy $status" >&2
  exit 1
fi

"$here/smoke.sh" "$commit"
