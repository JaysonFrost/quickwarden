#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ $# -gt 0 ]; then
  cargo build --release --locked --target "$1"
  bin="target/$1/release/quickwarden"
else
  cargo build --release
  bin="target/release/quickwarden"
fi
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
app=target/Quickwarden.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$bin" "$app/Contents/MacOS/quickwarden"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Quickwarden</string>
  <key>CFBundleIdentifier</key><string>io.github.jaysonfrost.quickwarden</string>
  <key>CFBundleExecutable</key><string>quickwarden</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>LSUIElement</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Reads the active app and browser tab to suggest matching logins.</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "$app"
