#!/usr/bin/env bash
# gh release create uploads assets to a draft before publishing. A tag push
# starts CI earlier; read-only tokens cannot download that draft's assets.
set -euo pipefail
tag=${1:?release tag required}
for ((attempt=1; attempt<=${RELEASE_WAIT_ATTEMPTS:-180}; attempt++)); do
  if [ "$(gh release view "$tag" --json isDraft --jq '.isDraft' 2>/dev/null || true)" = false ]; then
    echo "Release $tag is published; desktop assets are ready to download."
    exit 0
  fi
  sleep "${RELEASE_WAIT_SECONDS:-10}"
done
echo "::error::Release $tag was not published in time. Refusing to ship older downloads."
exit 1
