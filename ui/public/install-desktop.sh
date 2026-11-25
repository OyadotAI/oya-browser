#!/bin/sh
# Installs Oya Browser on this Mac when it is missing, opens it, and asks it to connect one coding
# agent. Usage: curl -fsSL https://oyabrowser.com/install-desktop.sh | sh -s -- claude-code
#
# It never connects anything by itself: Oya shows a dialog, and a person clicks Connect there.
# It never replaces an installed copy, never asks for sudo, and installs only an app that carries
# Oya's own Apple signature. curl sets no quarantine flag, so macOS would not check a copied app
# by itself; this script checks it before copying. Everything runs inside main, called on the
# last line, so a download cut short runs nothing.

set -eu

TEAM_ID="68267DX5UC"
APP_NAME="Oya Browser.app"
DOWNLOAD_URL="https://oyabrowser.com/download/mac"
WORK=""

fail() {
  echo "$1" >&2
  exit 1
}

cleanup() {
  [ -n "$WORK" ] || return 0
  hdiutil detach "$WORK/mount" -quiet >/dev/null 2>&1 || true
  rm -rf "$WORK"
}

# Prints the agent's name for an id, or fails before anything is downloaded.
agent_name() {
  case "$1" in
    claude-code) echo "Claude Code" ;;
    codex) echo "Codex" ;;
    cursor) echo "Cursor" ;;
    vscode) echo "VS Code" ;;
    "") fail "Name the agent to connect: claude-code, codex, cursor or vscode." ;;
    *) fail "Unknown agent \"$1\". Use one of: claude-code, codex, cursor, vscode." ;;
  esac
}

# Prints where Oya is installed, or nothing.
installed_app() {
  for dir in "/Applications" "$HOME/Applications"; do
    if [ -d "$dir/$APP_NAME" ]; then
      echo "$dir/$APP_NAME"
      return 0
    fi
  done
}

# Fails unless the app at $1 is signed by Oya and accepted by macOS.
verify_app() {
  codesign --verify --deep --strict "$1" >/dev/null 2>&1 || return 1
  team=$(codesign -dv "$1" 2>&1 | sed -n 's/^TeamIdentifier=//p')
  [ "$team" = "$TEAM_ID" ] || return 1
  spctl --assess --type execute "$1" >/dev/null 2>&1
}

# Downloads, checks and copies the app; prints nothing but progress. Sets APP.
install_app() {
  not_installed="Could not download Oya Browser. Check the connection and run this line again, or download it from https://oyabrowser.com/#download. Nothing was installed."
  echo "Downloading Oya Browser..."
  WORK=$(mktemp -d)
  curl -fsSL -o "$WORK/Oya.dmg" "$DOWNLOAD_URL" || fail "$not_installed"
  hdiutil attach "$WORK/Oya.dmg" -nobrowse -readonly -mountpoint "$WORK/mount" -quiet || fail "$not_installed"
  verify_app "$WORK/mount/$APP_NAME" ||
    fail "The download is not signed by Oya, so it was not installed. Download it from https://oyabrowser.com/#download. Nothing was installed."
  dest="/Applications"
  [ -w "$dest" ] || dest="$HOME/Applications"
  mkdir -p "$dest"
  echo "Installing to $dest..."
  rm -rf "$dest/.$APP_NAME.installing"
  ditto "$WORK/mount/$APP_NAME" "$dest/.$APP_NAME.installing"
  mv "$dest/.$APP_NAME.installing" "$dest/$APP_NAME"
  APP="$dest/$APP_NAME"
}

main() {
  agent="${1:-}"
  name=$(agent_name "$agent") || exit 1
  os=$(uname -s)
  [ "$os" = "Darwin" ] ||
    fail "Oya Browser's desktop app runs on macOS and Windows. This is $os. See https://oyabrowser.com/llms.txt for the cloud path."
  trap cleanup EXIT
  APP=$(installed_app)
  if [ -n "$APP" ]; then echo "Oya Browser is already installed."; else install_app; fi
  echo "Opening Oya Browser..."
  echo "Asking Oya to connect $name..."
  open -a "$APP" "oya://agent-connect?agent=$agent"
  echo "Oya Browser is showing a dialog. A person must click \"Connect $name\" there; an agent cannot answer it."
  echo "If no dialog appears, this copy of Oya is older: click Connect next to $name in the footer of the Oya window."
  echo "On a first install, Oya opens on its welcome screen: the person picks how to start there before the tools answer."
  echo "Then start a new $name session so the oya-browser tools appear."
}

main "$@"
