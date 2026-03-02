#!/bin/bash
set -e

echo "Building Simple Knotcoin DMG..."

# Create app structure
rm -rf /tmp/Knotcoin-simple
mkdir -p /tmp/Knotcoin-simple/Knotcoin.app/Contents/{MacOS,Resources}

# Copy binary
cp target/release/knotcoind /tmp/Knotcoin-simple/Knotcoin.app/Contents/MacOS/knotcoind
chmod +x /tmp/Knotcoin-simple/Knotcoin.app/Contents/MacOS/knotcoind

# Copy UI
cp -R share/explorer /tmp/Knotcoin-simple/Knotcoin.app/Contents/Resources/

# Copy icon
cp electron/icon.icns /tmp/Knotcoin-simple/Knotcoin.app/Contents/Resources/AppIcon.icns 2>/dev/null || true

# Create launcher script
cat > /tmp/Knotcoin-simple/Knotcoin.app/Contents/MacOS/Knotcoin << 'EOF'
#!/bin/bash
cd "$(dirname "$0")"
./knotcoind --rpc-port=9001 --p2p-port=9000 &
sleep 2
open "../Resources/explorer/index.html"
EOF

chmod +x /tmp/Knotcoin-simple/Knotcoin.app/Contents/MacOS/Knotcoin

# Create Info.plist
cat > /tmp/Knotcoin-simple/Knotcoin.app/Contents/Info.plist << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Knotcoin</string>
    <key>CFBundleIdentifier</key>
    <string>com.knotcoin.wallet</string>
    <key>CFBundleName</key>
    <string>Knotcoin</string>
    <key>CFBundleDisplayName</key>
    <string>Knotcoin</string>
    <key>CFBundleVersion</key>
    <string>1.0.2</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.2</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
</dict>
</plist>
EOF

# Create DMG
mkdir -p dist
rm -f dist/Knotcoin-1.0.2-macOS.dmg
hdiutil create -volname "Knotcoin" -srcfolder /tmp/Knotcoin-simple/Knotcoin.app -ov -format UDZO dist/Knotcoin-1.0.2-macOS.dmg

echo "Done! DMG created at: dist/Knotcoin-1.0.2-macOS.dmg"
ls -lh dist/Knotcoin-1.0.2-macOS.dmg
