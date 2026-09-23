#!/usr/bin/env bash
# Deploy Oya to Amazon ECS: control plane behind an HTTPS load balancer, RDS
# Postgres, and one Fargate task per browser, all managed through IAM roles.
# See README.md.
#
#   ./deploy.sh deploy --certificate-arn ARN [options]     (or --http-only to try it without a certificate)
#   ./deploy.sh smoke | status | keys | logs
#   ./deploy.sh destroy [--yes]
#
# Options for deploy:
#   --stack NAME            stack name (default oya)
#   --region REGION         (default: your AWS CLI region, else us-east-1)
#   --vpc-id ID --public-subnets A,B --private-subnets C,D
#                           your VPC; without these, a VPC is created as <stack>-network
#   --allowed-cidr CIDR     who may reach the load balancer (default 0.0.0.0/0)
#   --replicas N            control-plane replicas (default 2)
#   --db-class CLASS        RDS instance class (default db.t4g.small)
#   --single-az-db          no standby database (cheaper, not for production)
#   --browser-image REF     (default ghcr.io/oyadotai/oya-browser:latest, pinned by digest)
#   --server-image REF      a published Oya server image instead of building this checkout
#   --env-file FILE         optional settings, KEY=VALUE per line (default: oya.env here if it exists);
#                           see ../ENVIRONMENT.md for every variable
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../lib.sh
. "$HERE/../lib.sh"

STACK=oya
REGION=${AWS_REGION:-$(aws configure get region 2>/dev/null || true)}
REGION=${REGION:-us-east-1}

usage() { sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; }

# aws in this deployment's region.
awsr() { aws --region "$REGION" "$@"; }

# One output of a stack, or empty.
stack_output() {
  awsr cloudformation describe-stacks --stack-name "$1" \
    --query "Stacks[0].Outputs[?OutputKey=='$2'].OutputValue" --output text 2>/dev/null || true
}
stack_exists() { awsr cloudformation describe-stacks --stack-name "$1" >/dev/null 2>&1; }

# The control-plane secret's JSON.
secret_json() { awsr secretsmanager get-secret-value --secret-id "$STACK/control-plane" --query SecretString --output text; }
first_key() { secret_json | jq -r '.API_KEYS' | cut -d, -f1; }

# The architecture this machine builds natively, as Docker and Fargate name it.
native_arch() {
  case $(uname -m) in
    arm64 | aarch64) echo "linux/arm64 ARM64" ;;
    *) echo "linux/amd64 X86_64" ;;
  esac
}

# Sets VPC, PUBLIC_SUBNETS and PRIVATE_SUBNETS, creating <stack>-network when none was given.
ensure_network() {
  [ -n "$VPC" ] && return
  log "Creating the network stack $STACK-network (VPC, subnets in two zones, NAT gateways)"
  awsr cloudformation deploy --stack-name "$STACK-network" --template-file "$HERE/network.yaml" --no-fail-on-empty-changeset >/dev/null
  VPC=$(stack_output "$STACK-network" VpcId)
  PUBLIC_SUBNETS=$(stack_output "$STACK-network" PublicSubnetIds)
  PRIVATE_SUBNETS=$(stack_output "$STACK-network" PrivateSubnetIds)
}

# Builds the deployment image, pushes it to ECR and prints it pinned by digest.
push_image() {
  local platform=$1 server_image=$2 account repo tag
  account=$(aws sts get-caller-identity --query Account --output text)
  repo="$account.dkr.ecr.$REGION.amazonaws.com/$STACK/control-plane"
  awsr ecr describe-repositories --repository-names "$STACK/control-plane" >/dev/null 2>&1 ||
    awsr ecr create-repository --repository-name "$STACK/control-plane" --image-tag-mutability IMMUTABLE \
      --image-scanning-configuration scanOnPush=true >/dev/null
  tag="${repo}:$(image_tag)"
  build_image "$tag" "$platform" "$server_image"
  awsr ecr get-login-password | docker login --username AWS --password-stdin "${repo%%/*}" >/dev/null
  log "Pushing $tag"
  docker push -q "$tag" >/dev/null
  echo "${repo}@$(awsr ecr describe-images --repository-name "$STACK/control-plane" --image-ids imageTag="${tag##*:}" \
    --query 'imageDetails[0].imageDigest' --output text)"
}

# Writes the optional settings file into the <stack>/settings secret as one
# JSON object and prints the secret's ARN and a version that changes with it.
push_settings() {
  local file=$1 json arn
  json='{}'
  if [ -n "$file" ]; then
    [ -f "$file" ] || die "no settings file $file"
    json=$(settings_json <"$file")
    log "Settings: $(jq -r 'keys | join(", ")' <<<"$json")"
  fi
  if arn=$(awsr secretsmanager describe-secret --secret-id "$STACK/settings" --query ARN --output text 2>/dev/null); then
    awsr secretsmanager put-secret-value --secret-id "$arn" --secret-string "$json" >/dev/null
  else
    arn=$(awsr secretsmanager create-secret --name "$STACK/settings" --secret-string "$json" --query ARN --output text)
  fi
  echo "$arn $(printf '%s' "$json" | openssl dgst -sha256 | awk '{print substr($NF, 1, 16)}')"
}

# KEY=VALUE lines as a JSON object: comments and blank lines skipped, one pair of surrounding quotes removed.
settings_json() {
  jq -Rn '[inputs | select(test("^[A-Z_][A-Z0-9_]*=")) | capture("^(?<k>[^=]+)=(?<v>.*)$")
    | .v |= (sub("^\"(?<x>.*)\"$"; "\(.x)") | sub("^\u0027(?<x>.*)\u0027$"; "\(.x)"))
    | {(.k): .v}] | add // {}'
}

# The secrets, generated on the first deploy only: a redeploy keeps the stack's
# values, or stored credentials become unreadable.
secret_overrides() {
  stack_exists "$STACK" && return
  echo "ApiKeys=$(random_hex 24) ProfileSecret=$(random_hex 32) ClusterSecret=$(random_hex 32) DbPassword=$(random_hex 24)"
}

cmd_deploy() {
  local cert='' http_only=0 cidr=0.0.0.0/0 replicas=2 db_class=db.t4g.small multi_az=true
  local browser_image=ghcr.io/oyadotai/oya-browser:latest server_image='' platform arch image browser
  local env_file='' settings_arn settings_version
  [ -f "$HERE/oya.env" ] && env_file="$HERE/oya.env"
  VPC='' PUBLIC_SUBNETS='' PRIVATE_SUBNETS=''
  while [ $# -gt 0 ]; do
    case $1 in
      --stack) STACK=$2 && shift 2 ;;
      --region) REGION=$2 && shift 2 ;;
      --certificate-arn) cert=$2 && shift 2 ;;
      --http-only) http_only=1 && shift ;;
      --vpc-id) VPC=$2 && shift 2 ;;
      --public-subnets) PUBLIC_SUBNETS=$2 && shift 2 ;;
      --private-subnets) PRIVATE_SUBNETS=$2 && shift 2 ;;
      --allowed-cidr) cidr=$2 && shift 2 ;;
      --replicas) replicas=$2 && shift 2 ;;
      --db-class) db_class=$2 && shift 2 ;;
      --single-az-db) multi_az=false && shift ;;
      --browser-image) browser_image=$2 && shift 2 ;;
      --server-image) server_image=$2 && shift 2 ;;
      --env-file) env_file=$2 && shift 2 ;;
      *) die "unknown option $1 (see ./deploy.sh)" ;;
    esac
  done
  need aws docker jq openssl git
  [ -n "$cert" ] || [ "$http_only" = 1 ] || die "pass --certificate-arn for HTTPS, or --http-only to try it out without one"
  [ -z "$VPC" ] || { [ -n "$PUBLIC_SUBNETS" ] && [ -n "$PRIVATE_SUBNETS" ]; } || die "--vpc-id needs --public-subnets and --private-subnets"
  [ -n "$cert" ] || warn "no certificate: the load balancer serves plain HTTP; for trying it out only"
  log "Deploying stack $STACK to $REGION in account $(aws sts get-caller-identity --query Account --output text)"
  ensure_network
  read -r platform arch <<<"$(native_arch)"
  image=$(push_image "$platform" "$server_image")
  browser=$(pin_image "$browser_image")
  read -r settings_arn settings_version <<<"$(push_settings "$env_file")"
  log "Deploying the stack; the first deploy creates the database and takes about 15 minutes"
  # shellcheck disable=SC2046  # secret_overrides is a list of KEY=VALUE words
  awsr cloudformation deploy --stack-name "$STACK" --template-file "$HERE/template.yaml" \
    --capabilities CAPABILITY_IAM --no-fail-on-empty-changeset \
    --parameter-overrides VpcId="$VPC" PublicSubnetIds="$PUBLIC_SUBNETS" PrivateSubnetIds="$PRIVATE_SUBNETS" \
    CertificateArn="$cert" AllowedCidr="$cidr" Replicas="$replicas" DbInstanceClass="$db_class" DbMultiAz="$multi_az" \
    ControlPlaneImage="$image" ControlPlaneArchitecture="$arch" BrowserImage="$browser" \
    SettingsSecretArn="$settings_arn" SettingsVersion="$settings_version" $(secret_overrides)
  log "Waiting for the control plane to be healthy"
  awsr ecs wait services-stable --cluster "$STACK" --services control-plane
  log "Oya is up at $(stack_output "$STACK" Url)"
  log "API key: $(first_key)   (./deploy.sh keys shows it again)"
}

cmd_smoke() { smoke_test "$(stack_output "$STACK" Url)" "$(first_key)"; }

cmd_status() {
  log "Stack $STACK: $(awsr cloudformation describe-stacks --stack-name "$STACK" --query 'Stacks[0].StackStatus' --output text)"
  log "URL: $(stack_output "$STACK" Url)"
  awsr ecs describe-services --cluster "$STACK" --services control-plane \
    --query 'services[0].{running:runningCount,desired:desiredCount,deployments:length(deployments)}' --output table
  log "Browsers running: $(awsr ecs list-tasks --cluster "$STACK" --family "$STACK-worker" --query 'length(taskArns)')"
}

cmd_keys() { secret_json | jq -r '.API_KEYS' | tr ',' '\n'; }

cmd_destroy() {
  local db snapshot
  confirm "Delete stack $STACK in $REGION? The database is kept as a final snapshot."
  db=$(awsr cloudformation describe-stack-resource --stack-name "$STACK" --logical-resource-id Database \
    --query StackResourceDetail.PhysicalResourceId --output text 2>/dev/null || true)
  if [ -n "$db" ]; then
    log "Lifting deletion protection on database $db"
    awsr rds modify-db-instance --db-instance-identifier "$db" --no-deletion-protection --apply-immediately >/dev/null
  fi
  log "Stopping browsers"
  awsr ecs list-tasks --cluster "$STACK" --family "$STACK-worker" --query 'taskArns[]' --output text 2>/dev/null |
    tr '\t' '\n' | grep . | while read -r task; do awsr ecs stop-task --cluster "$STACK" --task "$task" >/dev/null; done || true
  log "Deleting stack $STACK (a few minutes; the database takes its final snapshot)"
  awsr cloudformation delete-stack --stack-name "$STACK"
  awsr cloudformation wait stack-delete-complete --stack-name "$STACK"
  awsr ecr delete-repository --repository-name "$STACK/control-plane" --force >/dev/null 2>&1 || true
  awsr secretsmanager delete-secret --secret-id "$STACK/settings" --force-delete-without-recovery >/dev/null 2>&1 || true
  if stack_exists "$STACK-network"; then
    log "Deleting the network stack $STACK-network"
    awsr cloudformation delete-stack --stack-name "$STACK-network"
    awsr cloudformation wait stack-delete-complete --stack-name "$STACK-network"
  fi
  if [ -n "$db" ]; then
    snapshot=$(awsr rds describe-db-snapshots --db-instance-identifier "$db" --snapshot-type manual \
      --query 'DBSnapshots[-1].DBSnapshotIdentifier' --output text 2>/dev/null || true)
    log "Kept the final database snapshot $snapshot; delete it with: aws rds delete-db-snapshot --db-snapshot-identifier $snapshot --region $REGION"
  fi
  log "Removed"
}

# Global options for the other commands: --stack, --region, --yes.
parse_globals() {
  while [ $# -gt 0 ]; do
    case $1 in
      --stack) STACK=$2 && shift 2 ;;
      --region) REGION=$2 && shift 2 ;;
      --yes) export ASSUME_YES=1 && shift ;;
      *) die "unknown option $1" ;;
    esac
  done
}

case ${1:-} in
  deploy) shift && cmd_deploy "$@" ;;
  smoke | status | keys | destroy) command=$1 && shift && parse_globals "$@" && "cmd_$command" ;;
  logs) shift && parse_globals "$@" && awsr logs tail "/oya/$STACK" --follow ;;
  *) usage && exit 1 ;;
esac
