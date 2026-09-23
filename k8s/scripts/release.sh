#!/bin/bash
# Build browser, update download links, create GitHub release, tag and push.
# Runs start to finish with no prompt. The tag's prod workflow publishes the
# npm SDK (@oya-ai/browser) and CLI (@oya-ai/cli) at the same version.
# Usage: ./release.sh [version] [--no-desktop]
#   ./release.sh             , auto-increments patch (v1.0.0 → v1.0.1)
#   ./release.sh 1.2.0       , tags as v1.2.0
#   ./release.sh --no-desktop, skip the macOS build (see below)
#
# Only macOS is built here; Linux and Windows are built by GitHub Actions on the
# tag either way. --no-desktop skips that local build, which is the slow part of
# a release that only changes the server, the console or the SDK. The prod image
# then carries the previous release's desktop assets forward, so downloads and
# auto-update keep working, they just stay on the version they already named.

set -e

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

log_info() { echo -e "\033[0;34m[INFO]\033[0m $1"; }
log_ok()   { echo -e "\033[0;32m[OK]\033[0m $1"; }
log_err()  { echo -e "\033[0;31m[ERR]\033[0m $1"; }

# ── Determine version ──

BUILD_DESKTOP=1
VERSION_ARG=""
for arg in "$@"; do
  case "$arg" in
    --no-desktop) BUILD_DESKTOP=0 ;;
    -*)           log_err "Unknown option: $arg"; exit 1 ;;
    *)            VERSION_ARG="$arg" ;;
  esac
done

# Local tags go stale (a fresh clone, a pruned tag) and the version is derived
# from them, so without this the script picks a version GitHub has already
# released, and only finds out after the commit, the tag and the branch push.
git fetch --tags --force --quiet origin
LATEST=$(git tag -l 'v*' --sort=-v:refname | head -1)

if [ -n "$VERSION_ARG" ]; then
  VERSION="${VERSION_ARG#v}"
  TAG="v${VERSION}"
else
  if [ -z "$LATEST" ]; then
    VERSION="1.0.0"
  else
    OLD="${LATEST#v}"
    IFS='.' read -r MAJOR MINOR PATCH <<< "$OLD"
    PATCH=$((PATCH + 1))
    VERSION="${MAJOR}.${MINOR}.${PATCH}"
  fi
  TAG="v${VERSION}"
fi

if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null || gh release view "$TAG" >/dev/null 2>&1; then
  log_err "$TAG already exists as a tag or a GitHub release. Pick a free version: make release V=x.y.z"
  exit 1
fi

log_info "Latest tag: ${LATEST:-none}"
log_info "New tag:    $TAG"
log_info "Version:    $VERSION"
log_info "Branch:     $(git branch --show-current)"
echo ""

# ── Preflight: notarization credentials ──

# The universal build takes ~5min and notarization runs at the very end, so a
# revoked app-specific password used to cost a full build before surfacing.
log_info "Checking Apple notarization credentials"
if ! xcrun notarytool history \
  --apple-id "$APPLE_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --team-id "$APPLE_TEAM_ID" \
  >/dev/null 2>&1; then
  log_err "Apple rejected APPLE_ID / APPLE_APP_SPECIFIC_PASSWORD / APPLE_TEAM_ID."
  log_err "App-specific passwords are revoked whenever the Apple ID password changes."
  log_err "Generate a new one at appleid.apple.com and update it in ~/.zshrc."
  exit 1
fi
log_ok "Notarization credentials valid"
echo ""

log_info "Releasing $TAG"

# ── Update browser/package.json version ──

log_info "Updating browser/package.json version to $VERSION"
cd "$ROOT/browser"
npm version "$VERSION" --no-git-tag-version --allow-same-version
cd "$ROOT"

# ── Update SDK and CLI versions ──

# The CLI pin moves first so that `npm version` rewrites the lockfile with it;
# in the other order the lock keeps the old pin until the next install.
log_info "Updating @oya-ai/browser and @oya-ai/cli to $VERSION"
npm pkg set "devDependencies.@oya-ai/browser=$VERSION" --workspace=@oya-ai/cli
npm version "$VERSION" --no-git-tag-version --allow-same-version \
  --workspace=@oya-ai/browser --workspace=@oya-ai/cli >/dev/null

# The two public manifests. Missed here, the Claude Code marketplace and the MCP
# registry keep advertising a version that has no tag, and the gap widens every
# release. (The prod workflow overwrites server.json's version from the tag when
# it publishes, but the committed file is what a reader sees.)
log_info "Updating .claude-plugin/plugin.json and server.json to $VERSION"
# A string replace, not parse-and-reserialize: these are hand-formatted files
# and JSON.stringify would reflow every array in them on each release.
node -e '
  const fs = require("fs");
  for (const f of [".claude-plugin/plugin.json", "server.json"]) {
    const before = fs.readFileSync(f, "utf8");
    const after = before.replace(/"version": "[^"]*"/, `"version": "${process.argv[1]}"`);
    if (after === before) { console.error(`${f}: no version field to update`); process.exit(1); }
    fs.writeFileSync(f, after);
  }
' "$VERSION"

# ── Build browser ──
#
# macOS only: Linux and Windows are built by GitHub Actions on the tag. This is
# the slow step, and a release that does not touch browser/ does not need it.

if [ "$BUILD_DESKTOP" = "0" ]; then
  log_info "Skipping the macOS build (--no-desktop)"
  log_info "Linux and Windows are still built by CI for this tag."
  log_info "Downloads and auto-update carry the previous desktop release forward,"
  log_info "so they keep working and keep naming that version."
else

  log_info "Building browser app..."
  cd "$ROOT/browser"

  log_info "Building macOS DMG (universal)"
  npm run dist:mac

  cd "$ROOT"

  # ── Copy macOS binary to server/downloads ──

  log_info "Copying binaries to server/downloads/"
  mkdir -p server/downloads

  # build.artifactName already emits the dotted name the download links use.
  SRC_DMG="browser/dist/Oya.Browser-${VERSION}-universal.dmg"
  DST_DMG="server/downloads/Oya.Browser-${VERSION}-universal.dmg"

  if [ -f "$SRC_DMG" ]; then
    cp "$SRC_DMG" "$DST_DMG"
    log_ok "Copied → $(basename "$DST_DMG")"
  else
    log_err "macOS DMG not found, check build output"
    exit 1
  fi

  # Auto-update reads latest-mac.yml and downloads the zip; Squirrel.Mac cannot
  # install from a DMG. Miss either and every client silently stops updating.
  SRC_ZIP="browser/dist/Oya.Browser-${VERSION}-universal.zip"
  SRC_YML="browser/dist/latest-mac.yml"
  for f in "$SRC_ZIP" "$SRC_YML"; do
    if [ ! -f "$f" ]; then
      log_err "$(basename "$f") not found, auto-update would be dead on this release"
      exit 1
    fi
  done
  log_ok "Update feed ready: $(basename "$SRC_ZIP") + latest-mac.yml"

  log_info "Linux AppImage will be built by GitHub Actions"

  # ── Update download links in UI ──

  log_info "Updating download links → $VERSION"

  # Every page that links a binary, not just the landing page, the docs page was
  # left out and sat three releases behind pointing at files CI no longer ships.
  UI_PAGES="ui/src/app/page.tsx ui/src/app/docs/page.tsx"
  for UI_PAGE in $UI_PAGES; do
    if [ -f "$UI_PAGE" ]; then
      sed -i.bak "s/Oya\.Browser-[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*-/Oya.Browser-${VERSION}-/g" "$UI_PAGE"
      rm -f "${UI_PAGE}.bak"
      log_ok "Updated $UI_PAGE"
    fi
  done
fi

# ── Commit, tag, push ──

log_info "Committing version bump and link updates"
git add browser/package.json browser/package-lock.json packages/sdk/package.json packages/cli/package.json package-lock.json \
  .claude-plugin/plugin.json server.json
for UI_PAGE in $UI_PAGES; do [ -f "$UI_PAGE" ] && git add "$UI_PAGE"; done
git commit -m "release: $TAG, update versions and download links"

git tag "$TAG"
log_ok "Tagged $TAG"

git push origin "$(git branch --show-current)"
log_ok "Pushed branch; publishing desktop assets before pushing the tag"

# ── Create GitHub release with binaries ──

log_info "Creating GitHub release $TAG..."

# With --no-desktop there are no local assets: CI attaches the Linux and
# Windows builds to this release, and the macOS binary stays on the release
# that last shipped one.
if [ "$BUILD_DESKTOP" = "0" ]; then
  gh release create "$TAG" --target "$(git rev-parse HEAD)" --title "Oya Browser $TAG" --generate-notes
  log_ok "GitHub release $TAG created (no desktop build in this one)"
else
  gh release create "$TAG" "$DST_DMG" "$SRC_ZIP" "$SRC_YML" --target "$(git rev-parse HEAD)" \
    --title "Oya Browser $TAG" \
    --generate-notes
  log_ok "GitHub release $TAG created with macOS binary and update feed"
fi
# Release creation may create the remote tag itself. Either way CI waits for
# publication before downloading, so it cannot mistake an uploading draft for
# a release without desktop assets.
git push origin "$TAG"
log_ok "Published release and pushed tag"

# ── SDK and CLI ──
#
# The prod workflow publishes both to npm once the deploy is green (the
# publish-npm job, npm Trusted Publishing): no login, no 2FA prompt here.

# ── Point cloud browsers at this release ──
#
# CI builds the browser image, registers snapshot oya-browser-<version>, and
# the deploy uses that job's output directly, so prod is already on the new
# snapshot by the time this runs. What is left is the DAYTONA_SNAPSHOT secret,
# which is the deploy's *fallback*.
#
# That is why this waits rather than setting it up front: the fallback must
# only ever name a snapshot that exists. Move it to a version whose
# registration then failed and a later deploy would send every cloud browser to
# a snapshot that was never created, instead of leaving them on the last build
# that worked.
#
# The release is complete at this point, interrupting here costs nothing but a
# stale fallback.

SNAPSHOT="oya-browser-${VERSION}"
JOB="Register Daytona snapshot"

log_info "Waiting for \"$JOB\" so the fallback can follow it..."

RUN_ID=""
for _ in $(seq 1 30); do
  RUN_ID=$(gh run list --workflow=deploy-prod.yaml --branch "$TAG" --limit 1 \
    --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)
  [ -n "$RUN_ID" ] && break
  sleep 5
done

if [ -z "$RUN_ID" ]; then
  log_err "No workflow run found for $TAG, leaving DAYTONA_SNAPSHOT alone."
  log_err "Once CI is green: gh secret set DAYTONA_SNAPSHOT --body \"$SNAPSHOT\""
  exit 0
fi

CONCLUSION=""
for _ in $(seq 1 90); do
  CONCLUSION=$(gh run view "$RUN_ID" --json jobs \
    --jq ".jobs[] | select(.name==\"$JOB\") | .conclusion" 2>/dev/null || true)
  [ -n "$CONCLUSION" ] && [ "$CONCLUSION" != "null" ] && break
  # The whole run finishing without that job ever reporting means it is not in
  # this workflow at all. Stop instead of waiting out the full timeout.
  if [ "$(gh run view "$RUN_ID" --json status --jq '.status' 2>/dev/null)" = "completed" ]; then
    break
  fi
  sleep 10
done

if [ "$CONCLUSION" = "success" ]; then
  gh secret set DAYTONA_SNAPSHOT --body "$SNAPSHOT"
  log_ok "Cloud browsers now default to $SNAPSHOT"
else
  log_err "\"$JOB\" did not succeed (${CONCLUSION:-timed out}), DAYTONA_SNAPSHOT left alone."
  log_err "Cloud browsers stay on the last snapshot that worked. Check: gh run view $RUN_ID"
fi
