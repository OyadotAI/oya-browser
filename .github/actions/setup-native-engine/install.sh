#!/usr/bin/env bash
# Install only a checksum-pinned native Oya distribution and verify its native capabilities.
set -euo pipefail
if [[ "$RUNNER_OS" != Linux || "$RUNNER_ARCH" != X64 ]]; then
  echo '::error::This native distribution requires a Linux X64 runner.'
  exit 1
fi
if [[ "$OYA_ENGINE_URL" != https://* || ! "$OYA_ENGINE_SHA256" =~ ^[[:xdigit:]]{64}$ ]]; then
  echo '::error::Configure OYA_NATIVE_LINUX_URL and OYA_NATIVE_LINUX_SHA256 with a pinned patched Oya distribution.'
  exit 1
fi
engine_dir=$(mktemp -d "$RUNNER_TEMP/oya-native.XXXXXX")
curl --fail --location --proto '=https' --proto-redir '=https' --retry 3 \
  --connect-timeout 30 --max-time 600 "$OYA_ENGINE_URL" --output "$engine_dir/runtime.tar.gz"
printf '%s  %s\n' "$OYA_ENGINE_SHA256" "$engine_dir/runtime.tar.gz" | sha256sum --check --status
mkdir "$engine_dir/distribution"
tar --extract --gzip --file "$engine_dir/runtime.tar.gz" \
  --directory "$engine_dir/distribution" --no-same-owner
test -x "$engine_dir/distribution/electron"
# Ubuntu 24.04 restricts unprivileged namespaces for binaries without a profile.
# Allow only this checksum-verified Oya executable; keep the renderer sandbox on.
if [[ "$(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || true)" == 1 ]]; then
  cat > "$engine_dir/apparmor.profile" <<EOF
abi <abi/4.0>,
include <tunables/global>
profile oya-native-${engine_dir##*/} "$engine_dir/distribution/electron" flags=(unconfined) {
  userns,
}
EOF
  sudo apparmor_parser --replace "$engine_dir/apparmor.profile"
fi
OYA_ENGINE_DIRECTORY="$engine_dir/distribution" xvfb-run -a node -e \
  'require("./browser/build/native-distribution.cjs").verify("linux", "x64", process.env.OYA_ENGINE_DIRECTORY)'
echo "OYA_NATIVE_ENGINE=$engine_dir/distribution/electron" >> "$GITHUB_ENV"
