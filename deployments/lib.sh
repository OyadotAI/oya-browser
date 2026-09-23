# shellcheck shell=bash
# Shared by every deployments/*/deploy.sh: logging, building the image, and
# the smoke test that proves a deployment starts and drives a browser.

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
export REPO_ROOT

# How long the smoke test waits for a new browser to connect.
SMOKE_CONNECT_TIMEOUT_S=${SMOKE_CONNECT_TIMEOUT_S:-240}

log() { printf '\033[1;34m==>\033[0m %s\n' "$*" >&2; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die() {
  printf '\033[1;31merror:\033[0m %s\n' "$*" >&2
  exit 1
}

# Fails unless every named command is installed.
need() {
  local cmd
  for cmd in "$@"; do command -v "$cmd" >/dev/null 2>&1 || die "$cmd is required but not installed"; done
}

# A random hex string of N bytes (default 32): URL-safe, so it can sit in a connection string.
random_hex() { openssl rand -hex "${1:-32}"; }

# Asks before something destructive, unless --yes was given (ASSUME_YES=1).
confirm() {
  [ "${ASSUME_YES:-0}" = 1 ] && return 0
  local answer
  read -r -p "$1 [y/N] " answer
  [[ $answer =~ ^[Yy]$ ]] || die "cancelled"
}

# A unique, sortable image tag: the time and the commit it was built from.
image_tag() {
  local rev
  rev=$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo nogit)
  git -C "$REPO_ROOT" diff --quiet 2>/dev/null || rev="$rev-dirty"
  echo "$(date -u +%Y%m%d%H%M%S)-$rev"
}

# build_image TAG PLATFORM [BASE_IMAGE]
# Builds the deployment image (deployments/image) as TAG. The Oya server under
# it is built from this checkout, unless BASE_IMAGE names a published one.
build_image() {
  local tag=$1 platform=$2 base=${3:-}
  if [ -z "$base" ]; then
    base="oya-server-base:${tag##*:}"
    build_server "$base" "$platform"
  fi
  log "Building the deployment image $tag"
  docker build --platform "$platform" --build-arg BASE_IMAGE="$base" \
    -f "$REPO_ROOT/deployments/image/Dockerfile" -t "$tag" "$REPO_ROOT"
}

# build_server TAG PLATFORM: the Oya server from the root Dockerfile.
build_server() {
  local tmp
  log "Building the Oya server from this checkout ($2); the first build takes a few minutes"
  tmp=$(mktemp -d)
  # server/downloads holds the desktop installers for the website's download
  # page, gigabytes a deployment never serves; keep them out of the context.
  cp "$REPO_ROOT/Dockerfile" "$tmp/Dockerfile"
  { cat "$REPO_ROOT/.dockerignore"; printf '\nserver/downloads/*\n!server/downloads/.gitkeep\n'; } >"$tmp/Dockerfile.dockerignore"
  docker build --platform "$2" -f "$tmp/Dockerfile" -t "$1" "$REPO_ROOT" || {
    rm -rf "$tmp"
    die "building the Oya server failed"
  }
  rm -rf "$tmp"
}

# The digest-pinned reference of a published image, so a deployment never
# changes versions when a tag moves.
pin_image() {
  local ref=$1 digest
  [[ $ref == *@sha256:* ]] && echo "$ref" && return
  digest=$(docker buildx imagetools inspect "$ref" --format '{{json .Manifest.Digest}}' | tr -d '"') ||
    die "could not resolve $ref"
  echo "${ref%%:*}@$digest"
}

# The platforms a published image is built for, one per line (e.g. linux/amd64).
image_platforms() {
  docker buildx imagetools inspect "$1" --raw | jq -r '
    if .manifests then .manifests[] | select(.platform.os != "unknown") | "\(.platform.os)/\(.platform.architecture)"
    else "linux/amd64" end'
}

# api METHOD PATH [JSON]: one call to the deployment's API as $SMOKE_URL with
# $SMOKE_KEY. The key goes through a header file, never argv.
api() {
  local header
  header=$(mktemp)
  printf 'Authorization: Bearer %s\n' "$SMOKE_KEY" >"$header"
  # shellcheck disable=SC2086  # SMOKE_CURL_OPTS is a list of flags, e.g. -k
  curl -sS --fail-with-body ${SMOKE_CURL_OPTS:-} -X "$1" -H "@$header" \
    -H 'Content-Type: application/json' ${3:+--data "$3"} "$SMOKE_URL$2"
  local status=$?
  rm -f "$header"
  return $status
}

# smoke_test URL KEY: starts an Oya Cloud browser, waits for it to connect,
# loads a page, and stops it. Exits non-zero on the first thing that fails.
smoke_test() {
  SMOKE_URL=${1%/} SMOKE_KEY=$2
  local id started=$SECONDS
  need curl jq
  api GET /health >/dev/null || die "$SMOKE_URL/health does not answer"
  log "Starting a browser"
  id=$(api POST /api/browsers/start '{"provider":"oya-cloud","name":"smoke test"}' | jq -r '.id // empty')
  [ -n "$id" ] || die "the start was refused"
  wait_connected "$id" || {
    api POST "/api/browsers/$id/stop" >/dev/null 2>&1
    die "browser $id did not connect within ${SMOKE_CONNECT_TIMEOUT_S}s"
  }
  log "Browser $id connected after $((SECONDS - started))s; loading a page"
  api POST "/api/browsers/$id/command" '{"action":"navigate","params":{"url":"https://example.com"}}' |
    jq -e '.ok == true and (.data.url | startswith("https://example.com"))' >/dev/null ||
    die "the browser did not load the page"
  api POST "/api/browsers/$id/stop" | jq -e '.ok == true' >/dev/null || die "stopping browser $id failed"
  log "Smoke test passed in $((SECONDS - started))s: started, connected, loaded a page, stopped"
}

# Waits until the browser shows as connected and healthy.
wait_connected() {
  local deadline=$((SECONDS + SMOKE_CONNECT_TIMEOUT_S))
  while [ $SECONDS -lt $deadline ]; do
    api GET /api/browsers | jq -e --arg id "$1" '.[] | select(.id == $id and .health == "ok")' >/dev/null 2>&1 && return 0
    sleep 3
  done
  return 1
}
