#!/bin/bash
set -e

echo "Building Knotcoin v1.0.3 packages for all platforms..."

VERSION="1.0.3"
BUILD_DIR="${BUILD_DIR:-/tmp/knotcoin-build}"
DIST_DIR="${DIST_DIR:-$(pwd)/dist}"

# Clean up
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"
mkdir -p "$DIST_DIR"

echo ""
echo "=== Building macOS DMG (Apple Silicon) ==="
echo ""

# Create macOS app bundle
APP_DIR="$BUILD_DIR/Knotcoin.app"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp electron/binaries/knotcoind-aarch64-apple-darwin "$APP_DIR/Contents/MacOS/knotcoind"
chmod +x "$APP_DIR/Contents/MacOS/knotcoind"

# Copy UI
cp -R share/explorer "$APP_DIR/Contents/Resources/"

# Copy icon
cp electron/icon.icns "$APP_DIR/Contents/Resources/icon.icns"

# Create launcher script
cat > "$APP_DIR/Contents/MacOS/Knotcoin" << 'LAUNCHER'
#!/bin/bash
cd "$(dirname "$0")/../Resources"
../MacOS/knotcoind --rpc-port=9001 --p2p-port=9000 &
KNOTCOIN_PID=$!
sleep 2
open "file://$(pwd)/explorer/index.html"
wait $KNOTCOIN_PID
LAUNCHER

chmod +x "$APP_DIR/Contents/MacOS/Knotcoin"

# Create Info.plist
cat > "$APP_DIR/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>Knotcoin</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>CFBundleIdentifier</key>
    <string>com.knotcoin.wallet</string>
    <key>CFBundleName</key>
    <string>Knotcoin</string>
    <key>CFBundleDisplayName</key>
    <string>Knotcoin</string>
    <key>CFBundleVersion</key>
    <string>1.0.3</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.3</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
</dict>
</plist>
PLIST

# Create DMG
echo "Creating DMG..."
rm -f "$DIST_DIR/Knotcoin-$VERSION-macOS-arm64.dmg"
hdiutil create -volname "Knotcoin" -srcfolder "$APP_DIR" -ov -format UDZO "$DIST_DIR/Knotcoin-$VERSION-macOS-arm64.dmg"

echo ""
echo "=== Building Windows ZIP ==="
echo ""

WIN_DIR="$BUILD_DIR/Knotcoin-Windows"
mkdir -p "$WIN_DIR"

# Copy binary
cp electron/binaries/knotcoind-x86_64-pc-windows-msvc.exe "$WIN_DIR/knotcoind.exe"

# Copy UI
cp -R share/explorer "$WIN_DIR/"

# Copy icon assets
cp electron/icon.ico "$WIN_DIR/Knotcoin.ico"

# Create launcher batch file
cat > "$WIN_DIR/Knotcoin.bat" << 'BATCH'
@echo off
cd /d "%~dp0"
start "" knotcoind.exe --rpc-port=9001 --p2p-port=9000
timeout /t 3 /nobreak >nul
start "" "explorer\index.html"
BATCH

# Create README
cat > "$WIN_DIR/README.txt" << 'README'
Knotcoin v1.0.3 for Windows

To start:
1. Double-click Knotcoin.bat
2. The node will start and UI will open in your browser

Data is stored in: C:\Users\<YourUsername>\.knotcoin\mainnet\

Icon: Knotcoin.ico (use it when creating a desktop shortcut)

To stop: Close the command window or use Task Manager to end knotcoind.exe
README

# Create ZIP
echo "Creating ZIP..."
(cd "$BUILD_DIR" && zip -r "$DIST_DIR/Knotcoin-$VERSION-Windows-x64.zip" "Knotcoin-Windows")

echo ""
echo "=== Building Linux TAR.GZ ==="
echo ""

LINUX_DIR="$BUILD_DIR/Knotcoin-Linux"
mkdir -p "$LINUX_DIR"

# Copy binary
cp electron/binaries/knotcoind-x86_64-unknown-linux-gnu "$LINUX_DIR/knotcoind"
chmod +x "$LINUX_DIR/knotcoind"

# Copy UI
cp -R share/explorer "$LINUX_DIR/"

# Copy icon assets
cp electron/icon.png "$LINUX_DIR/knotcoin.png"

# Create launcher script
cat > "$LINUX_DIR/knotcoin.sh" << 'LAUNCHER'
#!/bin/bash
cd "$(dirname "$0")"
./knotcoind --rpc-port=9001 --p2p-port=9000 &
KNOTCOIN_PID=$!
sleep 2
xdg-open "file://$(pwd)/explorer/index.html"
wait $KNOTCOIN_PID
LAUNCHER

chmod +x "$LINUX_DIR/knotcoin.sh"

# Create README
cat > "$LINUX_DIR/README.txt" << 'README'
Knotcoin v1.0.3 for Linux

To start:
1. Run: ./knotcoin.sh
2. The node will start and UI will open in your browser

Data is stored in: ~/.knotcoin/mainnet/

Icon: knotcoin.png (use it for desktop entries)

To stop: Press Ctrl+C in the terminal or kill the knotcoind process
README

# Create TAR.GZ
echo "Creating TAR.GZ..."
(cd "$BUILD_DIR" && tar -czf "$DIST_DIR/Knotcoin-$VERSION-Linux-x64.tar.gz" "Knotcoin-Linux")

echo ""
echo "=== Build Complete! ==="
echo ""
ls -lh "$DIST_DIR"/Knotcoin-$VERSION-*
echo ""
echo "Packages created:"
echo "  - macOS (Apple Silicon): $DIST_DIR/Knotcoin-$VERSION-macOS-arm64.dmg"
echo "  - Windows (x64):         $DIST_DIR/Knotcoin-$VERSION-Windows-x64.zip"
echo "  - Linux (x64):           $DIST_DIR/Knotcoin-$VERSION-Linux-x64.tar.gz"
echo ""
