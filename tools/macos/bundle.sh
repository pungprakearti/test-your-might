#!/usr/bin/env bash
# Wraps a macOS test-your-might binary in a minimal, ad-hoc-signed .app.
# Used by the release workflow and tools/update-e2e.sh.
#
#   tools/macos/bundle.sh <binary> <version> <out dir>
#       creates "<out dir>/Test Your Might.app"
#
# Not signed with a Developer ID or notarized (that needs a paid Apple
# developer account), so macOS asks the user to allow it on first launch -
# see README.
set -euo pipefail

binary=$1 version=$2 out=$3
app="$out/Test Your Might.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$binary" "$app/Contents/MacOS/test-your-might"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Test Your Might</string>
  <key>CFBundleDisplayName</key><string>Test Your Might</string>
  <key>CFBundleIdentifier</key><string>com.pungprakearti.test-your-might</string>
  <key>CFBundleExecutable</key><string>test-your-might</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist"
# Ad-hoc signature: required for arm64 binaries to run at all.
codesign --force --deep --sign - "$app"
