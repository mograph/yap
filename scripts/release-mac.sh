#!/usr/bin/env bash
# Builds Yap.dmg signed with your Developer ID and notarized by Apple, so it opens on any Mac
# without warnings.
#
# One-time setup:
#   1. Xcode → Settings → Accounts → your team → Manage Certificates → + → Developer ID Application
#   2. xcrun notarytool store-credentials yap --apple-id YOUR_APPLE_ID --team-id 6GAVK2Q4AG
#      (it asks for an app-specific password from appleid.apple.com)
#
# Then: ./scripts/release-mac.sh
set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY=$(security find-identity -v -p codesigning | grep -o '"Developer ID Application: [^"]*"' | head -1 | tr -d '"' || true)
if [ -z "$IDENTITY" ]; then
  echo "No Developer ID Application certificate yet. Do step 1 at the top of this script." >&2
  exit 1
fi
if ! xcrun notarytool history --keychain-profile yap >/dev/null 2>&1; then
  echo "No saved notarization credentials yet. Do step 2 at the top of this script." >&2
  exit 1
fi

echo "Signing with: $IDENTITY"
export APPLE_SIGNING_IDENTITY="$IDENTITY"
# CI=true skips the Finder window styling, which needs extra permissions.
CI=true pnpm tauri build --bundles dmg

DMG=$(ls -t src-tauri/target/release/bundle/dmg/*.dmg | head -1)
echo "Sending $DMG to Apple for notarization (usually a few minutes)…"
xcrun notarytool submit "$DMG" --keychain-profile yap --wait
xcrun stapler staple "$DMG"
spctl --assess --type open --context context:primary-signature --verbose "$DMG"
echo "Ready: $DMG"
