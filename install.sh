#!/bin/sh
# Self-host Oya Browser in one line:
#
#   curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh
#
# Checks for git, Docker and Node, clones (or updates) the repo into
# $OYA_DIR (default ~/oya-browser), then runs the `oya install` wizard there.
# Arguments pass through to the wizard:  ... | sh -s -- --dry-run
#
# For agents and CI, no questions at all (SQLite, Docker on this machine, one
# browser worker; an LLM only if OPENAI_API_KEY or ANTHROPIC_API_KEY is set):
#
#   curl -fsSL https://raw.githubusercontent.com/OyadotAI/oya-browser/main/install.sh | sh -s -- --yes
#
# With no terminal to ask on, --yes is added for you. OYA_YES=1 does the same.
set -eu

OYA_DIR="${OYA_DIR:-$HOME/oya-browser}"
REPO="https://github.com/OyadotAI/oya-browser.git"

say() { printf '\033[1;32m==>\033[0m %s\n' "$1"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required. $2"; }

need git "Install it from https://git-scm.com/downloads"
need docker "Install Docker Desktop (https://docs.docker.com/get-docker/) or Docker Engine."
# Docker Desktop on macOS can be started for the caller; elsewhere the daemon
# needs root, so say what to do instead.
if ! docker info >/dev/null 2>&1 && [ "$(uname)" = Darwin ] && open -a Docker 2>/dev/null; then
  say "Starting Docker Desktop"
  tries=0
  until docker info >/dev/null 2>&1 || [ "$tries" -ge 60 ]; do sleep 2; tries=$((tries + 1)); done
fi
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
# Piped into sh, stdin is this script; the wizard's questions need the terminal.
# Without one (an agent, CI) nobody can answer them, so take the defaults.
if [ "${OYA_YES:-}" != 1 ] && { : </dev/tty; } 2>/dev/null; then
  say "Starting the setup wizard"
  exec npx -y @oya-ai/cli@latest install "$@" </dev/tty
fi
case " $* " in *" --yes "*) ;; *) set -- --yes "$@" ;; esac
say "Installing with defaults: SQLite, Docker on this machine"
exec npx -y @oya-ai/cli@latest install "$@" </dev/null
