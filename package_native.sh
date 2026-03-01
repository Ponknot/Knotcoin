#!/bin/bash
# Fast native packaging for Knotcoin v1.0.2
# No slow electron-builder - just native tools

set -e

VERSION="1.0.2"
APP_NAME="Knotcoin"
BUNDLE_ID="com.knotcoin.wallet"

echo "🚀 Fast Native Packaging for Knotcoin v1.0.2"
echo "=============================================="

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# Create dist directory
mkdir -p dist

# ============================================
# macOS DMG (Apple Silicon)
# ============================================
build_macos_dmg() {
    echo -e "${BLUE}📦 Building macOS DMG (Apple Silicon)...${NC}"
    
    APP_DIR="dist/${APP_NAME}.app"
    
    # Clean old build
    rm -rf "$APP_DIR"
    
    # Create app bundle structure
    mkdir -p "$APP_DIR/Contents/MacOS"
    mkdir -p "$APP_DIR/Contents/Resources"
    
    # Copy binary
    cp electron/binaries/knotcoind-aarch64-apple-darwin "$APP_DIR/Contents/MacOS/knotcoind"
    chmod +x "$APP_DIR/Contents/MacOS/knotcoind"
    
    # Copy explorer UI
    cp -r share/explorer "$APP_DIR/Contents/Resources/"
    
    # Copy icon
    if [ -f "src-tauri/icons/icon.icns" ]; then
        cp src-tauri/icons/icon.icns "$APP_DIR/Contents/Resources/"
    fi
    
    # Create Info.plist
    cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>knotcoind</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>icon.icns</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
    
    # Create DMG using create-dmg (fast!)
    if command -v create-dmg &> /dev/null; then
        create-dmg \
            --volname "${APP_NAME} ${VERSION}" \
            --window-pos 200 120 \
            --window-size 800 400 \
            --icon-size 100 \
            --app-drop-link 600 185 \
            "dist/${APP_NAME}-${VERSION}-arm64.dmg" \
            "$APP_DIR" 2>/dev/null || {
                # Fallback: use hdiutil (always available on macOS)
                echo "Using hdiutil..."
                hdiutil create -volname "${APP_NAME}" -srcfolder "$APP_DIR" -ov -format UDZO "dist/${APP_NAME}-${VERSION}-arm64.dmg"
            }
    else
        # Use hdiutil (built-in macOS tool)
        hdiutil create -volname "${APP_NAME}" -srcfolder "$APP_DIR" -ov -format UDZO "dist/${APP_NAME}-${VERSION}-arm64.dmg"
    fi
    
    echo -e "${GREEN}✅ macOS DMG created: dist/${APP_NAME}-${VERSION}-arm64.dmg${NC}"
    ls -lh "dist/${APP_NAME}-${VERSION}-arm64.dmg"
}

# ============================================
# Windows EXE (using NSIS or zip)
# ============================================
build_windows_exe() {
    echo -e "${BLUE}📦 Building Windows installer...${NC}"
    
    WIN_DIR="dist/${APP_NAME}-win-x64"
    rm -rf "$WIN_DIR"
    mkdir -p "$WIN_DIR"
    
    # Copy binary
    cp electron/binaries/knotcoind-x86_64-pc-windows-msvc.exe "$WIN_DIR/knotcoind.exe"
    
    # Copy explorer UI
    cp -r share/explorer "$WIN_DIR/"
    
    # Create launcher script
    cat > "$WIN_DIR/${APP_NAME}.bat" << 'EOF'
@echo off
start "" "%~dp0knotcoind.exe"
EOF
    
    # Create README
    cat > "$WIN_DIR/README.txt" << EOF
Knotcoin v${VERSION}

To run:
1. Double-click ${APP_NAME}.bat
2. Open browser to http://localhost:19001

The node will run in the background.
EOF
    
    # Create zip
    (cd dist && zip -r "${APP_NAME}-${VERSION}-win-x64.zip" "${APP_NAME}-win-x64")
    
    echo -e "${GREEN}✅ Windows package created: dist/${APP_NAME}-${VERSION}-win-x64.zip${NC}"
    ls -lh "dist/${APP_NAME}-${VERSION}-win-x64.zip"
}

# ============================================
# Linux AppImage / Tarball
# ============================================
build_linux_package() {
    echo -e "${BLUE}📦 Building Linux package...${NC}"
    
    LINUX_DIR="dist/${APP_NAME}-linux-x64"
    rm -rf "$LINUX_DIR"
    mkdir -p "$LINUX_DIR"
    
    # Copy binary
    cp electron/binaries/knotcoind-x86_64-unknown-linux-gnu "$LINUX_DIR/knotcoind"
    chmod +x "$LINUX_DIR/knotcoind"
    
    # Copy explorer UI
    cp -r share/explorer "$LINUX_DIR/"
    
    # Create launcher script
    cat > "$LINUX_DIR/knotcoin.sh" << 'EOF'
#!/bin/bash
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$DIR/knotcoind" &
sleep 2
xdg-open http://localhost:19001 2>/dev/null || echo "Open http://localhost:19001 in your browser"
EOF
    chmod +x "$LINUX_DIR/knotcoin.sh"
    
    # Create README
    cat > "$LINUX_DIR/README.txt" << EOF
Knotcoin v${VERSION}

To run:
./knotcoin.sh

Or manually:
./knotcoind &
# Then open http://localhost:19001 in browser
EOF
    
    # Create tarball
    (cd dist && tar czf "${APP_NAME}-${VERSION}-linux-x64.tar.gz" "${APP_NAME}-linux-x64")
    
    echo -e "${GREEN}✅ Linux package created: dist/${APP_NAME}-${VERSION}-linux-x64.tar.gz${NC}"
    ls -lh "dist/${APP_NAME}-${VERSION}-linux-x64.tar.gz"
}

# ============================================
# Main
# ============================================

case "${1:-all}" in
    mac|macos|dmg)
        build_macos_dmg
        ;;
    win|windows|exe)
        build_windows_exe
        ;;
    linux)
        build_linux_package
        ;;
    all)
        build_macos_dmg
        build_windows_exe
        build_linux_package
        ;;
    *)
        echo "Usage: $0 [mac|win|linux|all]"
        exit 1
        ;;
esac

echo ""
echo -e "${GREEN}🎉 Build complete!${NC}"
echo ""
echo "📦 Packages created in dist/:"
ls -lh dist/*.dmg dist/*.zip dist/*.tar.gz 2>/dev/null || true
echo ""
echo "Next steps:"
echo "1. Test the packages"
echo "2. Create checksums: shasum -a 256 dist/* > dist/SHA256SUMS"
echo "3. Upload to GitHub Release"
