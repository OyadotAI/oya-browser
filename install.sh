#!/bin/sh
# Self-host Oya Browser in one line:
#
#   curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh
#
# Checks for git, Docker and Node, clones (or updates) the repo into
# $OYA_DIR (default ~/oya-browser), then runs the `oya install` wizard there.
# Arguments pass through to the wizard:  ... | sh -s -- --dry-run
set -eu

OYA_DIR="${OYA_DIR:-$HOME/oya-browser}"
REPO="https://github.com/OyadotAI/oya-browser.git"

say() { printf '\033[1;32m==>\033[0m %s\n' "$1"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required. $2"; }

need git "Install it from https://git-scm.com/downloads"
need docker "Install Docker Desktop (https://docs.docker.com/get-docker/) or Docker Engine."
docker info >/dev/null 2>&1 || die "Docker is installed but not running. Start it, then run this again."
need node "Install Node 20 or newer from https://nodejs.org"
node -e 'process.exit(+process.versions.node.split(".")[0] >= 20 ? 0 : 1)' \
  || die "Node $(node -v) is too old. Install Node 20 or newer from https://nodejs.org"

if [ -d "$OYA_DIR/.git" ]; then
  say "Updating $OYA_DIR"
  git -C "$OYA_DIR" pull --ff-only --quiet || say "Could not update it; using it as it is"
elif [ -e "$OYA_DIR" ]; then
  die "$OYA_DIR exists and is not a checkout. Set OYA_DIR to another folder."
else
  say "Cloning Oya Browser into $OYA_DIR"
  git clone --depth 1 --quiet "$REPO" "$OYA_DIR"
fi

cd "$OYA_DIR"
say "Starting the setup wizard"
# Piped into sh, stdin is this script; the wizard's questions need the terminal.
if { : </dev/tty; } 2>/dev/null; then exec npx -y @oya-ai/cli@latest install "$@" </dev/tty; fi
exec npx -y @oya-ai/cli@latest install "$@"
