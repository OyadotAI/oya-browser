# shellcheck shell=bash
# Shared by every deployments/*/deploy.sh: logging, building the image, and
# the smoke test that proves a deployment starts and drives a browser.

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
export REPO_ROOT

# How long the smoke test waits for a new browser to connect.
SMOKE_CONNECT_TIMEOUT_S=${SMOKE_CONNECT_TIMEOUT_S:-240}
# Readiness, individual HTTP calls and polling each have a bound.
SMOKE_READY_TIMEOUT_S=${SMOKE_READY_TIMEOUT_S:-180}
SMOKE_REQUEST_TIMEOUT_S=${SMOKE_REQUEST_TIMEOUT_S:-30}
SMOKE_CONNECT_DEADLINE_S=${SMOKE_CONNECT_DEADLINE_S:-5}
SMOKE_POLL_S=${SMOKE_POLL_S:-3}
# Stopping a disposable smoke browser must fit inside the installer's cancellation grace.
SMOKE_CLEANUP_TIMEOUT_S=${SMOKE_CLEANUP_TIMEOUT_S:-5}

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
  local ref=$1 digest name
  [[ $ref == *@sha256:* ]] && echo "$ref" && return
  digest=$(docker buildx imagetools inspect "$ref" --format '{{json .Manifest.Digest}}' | tr -d '"') ||
    die "could not resolve $ref"
  name=${ref%@*}
  [[ ${name##*/} != *:* ]] || name=${name%:*}
  echo "$name@$digest"
}

# The platforms a published image is built for, one per line (e.g. linux/amd64).
image_platforms() {
  docker buildx imagetools inspect "$1" --raw | jq -r '
    if .manifests then .manifests[] | select(.platform.os != "unknown") | "\(.platform.os)/\(.platform.architecture)"
    else "linux/amd64" end'
}

# api METHOD PATH [JSON]: one call to the deployment's API as $SMOKE_URL with
# $SMOKE_KEY. The key goes through a header file, never argv.
api() (
  local status=0
  SMOKE_API_HEADER=$(mktemp)
  trap 'rm -f "$SMOKE_API_HEADER"' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  printf 'Authorization: Bearer %s\n' "$SMOKE_KEY" >"$SMOKE_API_HEADER"
  # shellcheck disable=SC2086  # SMOKE_CURL_OPTS is a list of flags, e.g. -k
  curl -sS --fail-with-body --connect-timeout "$SMOKE_CONNECT_DEADLINE_S" --max-time "$SMOKE_REQUEST_TIMEOUT_S" \
    ${SMOKE_CURL_OPTS:-} -X "$1" -H "@$SMOKE_API_HEADER" \
    -H 'Content-Type: application/json' ${3:+--data "$3"} "$SMOKE_URL$2" || status=$?
  rm -f "$SMOKE_API_HEADER"
  return $status
)

# Own both the temporary browser and the optional port-forward, even when die exits.
cleanup_smoke() {
  if [ -n "${SMOKE_BROWSER_ID:-}" ]; then
    SMOKE_REQUEST_TIMEOUT_S=$SMOKE_CLEANUP_TIMEOUT_S api POST "/api/browsers/$SMOKE_BROWSER_ID/stop" >/dev/null 2>&1 || true
    SMOKE_BROWSER_ID=''
  fi
  if [ -n "${SMOKE_FORWARD_PID:-}" ]; then
    kill "$SMOKE_FORWARD_PID" 2>/dev/null || true
    wait "$SMOKE_FORWARD_PID" 2>/dev/null || true
    SMOKE_FORWARD_PID=''
  fi
}

# A RETURN trap misses exit from die; EXIT also runs on signal-triggered exits.
install_smoke_cleanup() {
  trap cleanup_smoke EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
}

# Liveness is insufficient: the database and background workers must be ready too.
wait_ready() {
  local deadline=$((SECONDS + SMOKE_READY_TIMEOUT_S))
  while [ "$SECONDS" -lt "$deadline" ]; do
    api GET /readyz >/dev/null 2>&1 && return 0
    sleep "$SMOKE_POLL_S"
  done
  return 1
}

# smoke_test URL KEY: starts an Oya Cloud browser, waits for it to connect,
# loads a page, and stops it. Exits non-zero on the first thing that fails.
smoke_test() {
  SMOKE_URL=${1%/} SMOKE_KEY=$2
  local id started=$SECONDS
  install_smoke_cleanup
  need curl jq
  log "Waiting for control-plane readiness"
  wait_ready || die "$SMOKE_URL/readyz did not become ready within ${SMOKE_READY_TIMEOUT_S}s"
  log "Starting a browser"
  # A deferred signal may run immediately after this assignment: publish ownership first.
  SMOKE_BROWSER_ID=$(api POST /api/browsers/start '{"provider":"oya-cloud","name":"smoke test"}' | jq -r '.id // empty')
  id=$SMOKE_BROWSER_ID
  [ -n "$id" ] || die "the start was refused"
  wait_connected "$id" || die "browser $id did not connect within ${SMOKE_CONNECT_TIMEOUT_S}s"
  log "Browser $id connected after $((SECONDS - started))s; loading a page"
  api POST "/api/browsers/$id/command" '{"action":"navigate","params":{"url":"https://example.com"}}' |
    jq -e '.ok == true and (.data.url | startswith("https://example.com"))' >/dev/null ||
    die "the browser did not load the page"
  api POST "/api/browsers/$id/stop" | jq -e '.ok == true' >/dev/null || die "stopping browser $id failed"
  SMOKE_BROWSER_ID=''
  cleanup_smoke
  log "Smoke test passed in $((SECONDS - started))s: started, connected, loaded a page, stopped"
}

# Waits until the browser shows as connected and healthy.
wait_connected() {
  local deadline=$((SECONDS + SMOKE_CONNECT_TIMEOUT_S))
  while [ $SECONDS -lt $deadline ]; do
    api GET /api/browsers | jq -e --arg id "$1" '.[] | select(.id == $id and .health == "ok")' >/dev/null 2>&1 && return 0
    sleep "$SMOKE_POLL_S"
  done
  return 1
}
