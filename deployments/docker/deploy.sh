#!/usr/bin/env bash
# Deploy Oya to one Docker host: control plane, Postgres, Caddy (HTTPS), and
# one browser container per browser. See README.md.
#
#   ./deploy.sh deploy [--domain oya.example.com] [--browser-image REF] [--server-image REF]
#                      optional settings: oya.env next to this script (see ../ENVIRONMENT.md)
#   ./deploy.sh smoke | status | logs | keys
#   ./deploy.sh destroy [--volumes] [--yes]
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
# shellcheck source-path=SCRIPTDIR source=../lib.sh
. "$HERE/../lib.sh"

ENV_FILE="$HERE/.env"
DEFAULT_BROWSER_IMAGE=ghcr.io/oyadotai/oya-browser:latest

usage() { sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; }

# docker compose for this deployment.
compose() { docker compose --project-directory "$HERE" -f "$HERE/compose.yaml" --env-file "$ENV_FILE" "$@"; }

# The value of KEY in .env, or empty.
env_get() { [ -f "$ENV_FILE" ] && grep -E "^$1=" "$ENV_FILE" | tail -1 | cut -d= -f2- || true; }

# Sets KEY=VALUE in .env, replacing any earlier value.
env_set() {
  local tmp
  tmp=$(mktemp)
  { [ -f "$ENV_FILE" ] && grep -vE "^$1=" "$ENV_FILE"; printf '%s=%s\n' "$1" "$2"; } >"$tmp" || true
  mv "$tmp" "$ENV_FILE"
  chmod 600 "$ENV_FILE"
}

# Generates each secret once; a redeploy keeps them, or stored credentials become unreadable.
ensure_secrets() {
  [ -n "$(env_get API_KEYS)" ] || env_set API_KEYS "$(random_hex 24)"
  [ -n "$(env_get OYA_PROFILE_SECRET)" ] || env_set OYA_PROFILE_SECRET "$(random_hex 32)"
  [ -n "$(env_get POSTGRES_PASSWORD)" ] || env_set POSTGRES_PASSWORD "$(random_hex 24)"
}

# This host's platform, e.g. linux/arm64.
host_platform() {
  case $(docker info --format '{{.Architecture}}') in
    aarch64 | arm64) echo linux/arm64 ;;
    *) echo linux/amd64 ;;
  esac
}

# The URL clients use, and curl flags for Caddy's local certificate.
public_url() { echo "https://$(env_get OYA_DOMAIN)"; }
local_tls_opts() { [ "$(env_get OYA_DOMAIN)" = localhost ] && echo "-k" || true; }

# Runs browsers natively when the image has a build for this host, emulated otherwise.
choose_browser_platform() {
  local host platforms
  host=$(host_platform)
  platforms=$(image_platforms "$(env_get OYA_BROWSER_IMAGE)")
  if grep -qx "$host" <<<"$platforms"; then
    env_set OYA_BROWSER_PLATFORM ''
    OYA_PULL_PLATFORM=$host
  else
    OYA_PULL_PLATFORM=$(head -1 <<<"$platforms")
    env_set OYA_BROWSER_PLATFORM "$OYA_PULL_PLATFORM"
    warn "the browser image has no $host build; browsers run as $OYA_PULL_PLATFORM under emulation, which is slower"
  fi
}

cmd_deploy() {
  local domain='' browser_image=$DEFAULT_BROWSER_IMAGE server_image='' tag
  while [ $# -gt 0 ]; do
    case $1 in
      --domain) domain=$2 && shift 2 ;;
      --browser-image) browser_image=$2 && shift 2 ;;
      --server-image) server_image=$2 && shift 2 ;;
      *) die "unknown option $1" ;;
    esac
  done
  need docker openssl git
  ensure_secrets
  env_set OYA_DOMAIN "${domain:-$(env_get OYA_DOMAIN)}"
  [ -n "$(env_get OYA_DOMAIN)" ] || env_set OYA_DOMAIN localhost
  [ "$(env_get OYA_DOMAIN)" = localhost ] && warn "no --domain: serving https://localhost with a local certificate, for trying it out only"
  env_set OYA_BROWSER_IMAGE "$(pin_image "$browser_image")"
  choose_browser_platform
  tag="oya-control-plane:$(image_tag)"
  build_image "$tag" "$(host_platform)" "$server_image"
  env_set OYA_IMAGE "$tag"
  log "Pulling the browser image $(env_get OYA_BROWSER_IMAGE)"
  docker pull -q --platform "${OYA_PULL_PLATFORM}" "$(env_get OYA_BROWSER_IMAGE)" >/dev/null
  log "Starting the stack"
  compose up -d --wait
  log "Oya is up at $(public_url)"
  log "API key: $(env_get API_KEYS | cut -d, -f1)   (./deploy.sh keys shows it again)"
}

cmd_smoke() {
  SMOKE_CURL_OPTS=$(local_tls_opts) smoke_test "$(public_url)" "$(env_get API_KEYS | cut -d, -f1)"
}

cmd_status() {
  compose ps
  echo
  log "Browser containers:"
  docker ps --filter label=oya-browser=true --format '  {{.Names}}  {{.Status}}  {{.Label "oya-name"}}'
}

cmd_keys() { env_get API_KEYS | tr ',' '\n'; }

cmd_destroy() {
  local volumes=''
  while [ $# -gt 0 ]; do
    case $1 in
      --volumes) volumes=--volumes && shift ;;
      --yes) export ASSUME_YES=1 && shift ;;
      *) die "unknown option $1" ;;
    esac
  done
  if [ -n "$volumes" ]; then
    confirm "Delete the stack AND its data (database, personas, cookies)?"
  else
    confirm "Stop and remove the stack (data volumes are kept)?"
  fi
  log "Stopping browser containers"
  docker ps -aq --filter label=oya-browser=true | xargs -r docker rm -f >/dev/null
  compose down $volumes
  [ -n "$volumes" ] && rm -f "$ENV_FILE"
  log "Removed"
}

case ${1:-} in
  deploy | smoke | status | keys | destroy) command=$1 && shift && "cmd_$command" "$@" ;;
  logs) shift && compose logs -f "$@" ;;
  *) usage && exit 1 ;;
esac
