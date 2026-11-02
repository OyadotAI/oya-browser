#!/bin/bash
# Re-signs dist/Mars.app with a Developer ID, rebuilds the DMG, notarizes and staples both.
# Run build-app.sh first. Nothing runs without an explicit identity and notary profile, so this
# script never signs with, or uploads under, an account the caller did not name.
#
# Usage: packaging/sign-release.sh "Developer ID Application: Name (TEAMID)" NOTARY_PROFILE
#   NOTARY_PROFILE is a keychain profile created once with:
#   xcrun notarytool store-credentials NOTARY_PROFILE --apple-id ... --team-id ...
#   Pass --no-notarize as the second argument to sign only.
set -euo pipefail

IDENTITY="${1:?Pass the Developer ID Application identity to sign with}"
PROFILE="${2:?Pass a notarytool keychain profile, or --no-notarize}"
PACKAGING="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MARS="$(dirname "$PACKAGING")"
APP="$MARS/dist/Mars.app"
VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
DMG="$MARS/dist/Mars-$VERSION.dmg"

codesign --force --sign "$IDENTITY" --timestamp --options runtime \
    --entitlements "$PACKAGING/entitlements.plist" --identifier ai.getoya.mars "$APP"
codesign --verify --strict --verbose=1 "$APP"

STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -quiet -volname "Mars $VERSION" -srcfolder "$STAGE" -format UDZO -ov "$DMG"
rm -rf "$STAGE"
codesign --force --sign "$IDENTITY" --timestamp "$DMG"

if [[ "$PROFILE" == "--no-notarize" ]]; then
    echo "Signed $APP and $DMG (not notarized)"
    exit 0
fi
xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$DMG"
xcrun stapler staple "$APP"
spctl --assess --type execute --verbose "$APP"
echo "Signed, notarized and stapled $APP and $DMG"
