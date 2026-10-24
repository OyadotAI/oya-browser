#!/bin/bash
# Builds a release mars binary and assembles a double-clickable dist/Mars.app plus dist/Mars.dmg.
# Signing is ad hoc (`codesign -s -`) so the app runs on this Mac; sign-release.sh re-signs with a
# Developer ID and notarizes when a release identity is supplied. Nothing here is uploaded.
#
# Usage: packaging/build-app.sh            (from anywhere)
# Env:   CARGO_TARGET_DIR  cargo output folder (default: mars/target)
#        MARS_SKIP_BUILD=1 reuse the existing release binary
set -euo pipefail

PACKAGING="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARS="$(dirname "$PACKAGING")"
REPO="$(dirname "$MARS")"
TARGET="${CARGO_TARGET_DIR:-$MARS/target}"
DIST="$MARS/dist"
APP="$DIST/Mars.app"
ICON_SOURCE="$REPO/browser/build/icon_1024.png"
VERSION="$(awk -F'"' '/^\[package\]/{p=1} p && /^version *=/{print $2; exit}' "$MARS/Cargo.toml")"

if [[ "${MARS_SKIP_BUILD:-0}" != 1 ]]; then
    cargo build --release --manifest-path "$MARS/Cargo.toml" --bin mars
fi
BINARY="$TARGET/release/mars"
[[ -x "$BINARY" ]] || { echo "No release binary at $BINARY" >&2; exit 1; }

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/mars"
sed "s/@VERSION@/$VERSION/g" "$PACKAGING/Info.plist" > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"
plutil -lint "$APP/Contents/Info.plist" >/dev/null

# The Oya mark at every size macOS asks for, compiled into one .icns.
ICONSET="$(mktemp -d)/Mars.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$ICON_SOURCE" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$ICON_SOURCE" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Mars.icns"
rm -rf "$(dirname "$ICONSET")"

# A stable local identity (make-dev-cert.sh) keeps Keychain grants across rebuilds; otherwise
# fall back to an ad hoc signature, which macOS treats as a new app after every build.
SIGN_IDENTITY="${MARS_SIGN_IDENTITY:-Mars Local Development}"
if ! security find-identity -v -p codesigning | grep -q "\"$SIGN_IDENTITY\""; then
    SIGN_IDENTITY="-"
fi
codesign --force --sign "$SIGN_IDENTITY" --options runtime \
    --entitlements "$PACKAGING/entitlements.plist" --identifier ai.getoya.mars "$APP"
codesign --verify --strict --verbose=1 "$APP"

DMG="$DIST/Mars-$VERSION.dmg"
STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "Mars $VERSION" -srcfolder "$STAGE" -format UDZO -ov "$DMG"
rm -rf "$STAGE"

echo "Built $APP ($VERSION)"
echo "Built $DMG"
