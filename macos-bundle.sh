#!/bin/sh
set -eu
cd "$(dirname "$0")"
cargo build --release
app=target/bwquick.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/bwquick "$app/Contents/MacOS/bwquick"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>bwquick</string>
  <key>CFBundleIdentifier</key><string>local.bwquick</string>
  <key>CFBundleExecutable</key><string>bwquick</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSUIElement</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Reads the active app and browser tab to suggest matching logins.</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "$app"
