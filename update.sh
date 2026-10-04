#!/bin/sh
# Update a self-hosted Oya Browser to the latest version:
#
#   curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/update.sh | sh
#
# For an install made with install.sh. Pulls the latest code into $OYA_DIR
# (default ~/oya-browser), rebuilds the images and restarts everything with
# the settings already in .env, keeping the number of browser workers running
# now. Nothing is asked. It stops, and changes nothing, when the checkout has
# local edits, so an update can never quietly leave the old code running.
#
#   ... | sh -s -- --force    rebuild and restart even when already up to date
set -eu

OYA_DIR="${OYA_DIR:-$HOME/oya-browser}"
GOVERNED_IMAGE="oya-browser:local"
READY_TRIES=90

say() { printf '\033[1;32m==>\033[0m %s\n' "$1"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }

FORCE=0
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=1 ;;
    *) die "Unknown option: $arg (the only option is --force)" ;;
  esac
done

# Run from inside the checkout, the pull below could rewrite this very file
# while sh is still reading it. Run a copy instead.
case "$0" in
  "$OYA_DIR"/*)
    copy="$(mktemp)"
    cp "$0" "$copy"
    exec sh "$copy" "$@"
    ;;
esac

command -v git >/dev/null 2>&1 || die "git is required."
command -v docker >/dev/null 2>&1 || die "Docker is required."
command -v curl >/dev/null 2>&1 || die "curl is required."
docker info >/dev/null 2>&1 || die "Docker is not running. Start it, then run this again."
[ -d "$OYA_DIR/.git" ] || die "No install found at $OYA_DIR. Install first (install.sh), or set OYA_DIR."
cd "$OYA_DIR"
[ -f .env ] || die "$OYA_DIR has no .env, so it was never set up. Run install.sh instead."

# The wizard can also set up Kubernetes or a server on this machine without
# Docker Compose; those are updated by running install.sh again.
if ! docker compose ps --quiet server 2>/dev/null | grep -q .; then
  die "Oya is not running under Docker Compose in $OYA_DIR. To update another kind of install, run install.sh again."
fi

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  git status --short --untracked-files=no
  die "$OYA_DIR has local changes (above), so nothing was updated. Move them aside (git -C $OYA_DIR stash) and run this again."
fi

before="$(git rev-parse --short HEAD)"
say "Fetching the latest version"
git pull --ff-only --quiet || die "Could not fast-forward $OYA_DIR to the latest version. Nothing was changed."
after="$(git rev-parse --short HEAD)"

if [ "$before" = "$after" ] && [ "$FORCE" != 1 ]; then
  say "Already up to date ($after). Run with --force to rebuild and restart anyway."
  exit 0
fi
[ "$before" = "$after" ] || say "Updating $before -> $after"

# Keep as many browser workers as run now (the wizard may have scaled them,
# or set them to 0 for governed browsers).
workers="$(docker compose ps --quiet browser 2>/dev/null | grep -c . || true)"

if docker image inspect "$GOVERNED_IMAGE" >/dev/null 2>&1; then
  say "Rebuilding the governed browser image"
  docker build --quiet -t "$GOVERNED_IMAGE" browser >/dev/null
fi

say "Rebuilding and restarting (browser workers: $workers)"
docker compose up -d --build --remove-orphans --scale "browser=$workers"

port="$(sed -n 's/^PORT=//p' .env | tail -n 1)"
url="http://127.0.0.1:${port:-3100}"
printf '  waiting for %s/readyz ' "$url"
tries=0
until curl -fsS "$url/readyz" >/dev/null 2>&1; do
  tries=$((tries + 1))
  [ "$tries" -lt "$READY_TRIES" ] || { echo; die "The server did not come back. Check: docker compose -f $OYA_DIR/docker-compose.yml logs -f server"; }
  printf '.'
  sleep 2
done
echo ' ready'
say "Oya Browser is up to date ($after)."
