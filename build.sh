#!/usr/bin/env bash
set -euo pipefail

# Knotcoin v1.0.2 — Cross-Platform Build Script
# Usage: ./build.sh [macos|linux|windows|all]

VERSION="1.0.2"
PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
ELECTRON_DIR="$PROJECT_ROOT/electron"
BINARIES_DIR="$ELECTRON_DIR/binaries"
DIST_DIR="$ELECTRON_DIR/dist"

mkdir -p "$BINARIES_DIR" "$DIST_DIR"

build_rust() {
    local target="$1"
    local output_name="$2"
    echo "==> Building knotcoind for $target..."

    if command -v cargo-zigbuild &>/dev/null; then
        cargo zigbuild --release --target "$target"
    else
        cargo build --release --target "$target"
    fi

    local src="$PROJECT_ROOT/target/$target/release/$output_name"
    cp "$src" "$BINARIES_DIR/"
    echo "    ✓ $output_name → $BINARIES_DIR/"
}

build_macos() {
    echo "=== macOS ARM64 ==="
    cargo build --release
    cp "$PROJECT_ROOT/target/release/knotcoind" "$BINARIES_DIR/knotcoind"
    cd "$ELECTRON_DIR" && npm install && npm run dist:mac:arm64
    echo "    ✓ DMG: $DIST_DIR/Knotcoin-$VERSION-arm64.dmg"
}

build_linux() {
    echo "=== Linux x86_64 ==="
    build_rust "x86_64-unknown-linux-gnu" "knotcoind"
    cp "$PROJECT_ROOT/target/x86_64-unknown-linux-gnu/release/knotcoind" \
       "$BINARIES_DIR/knotcoind-x86_64-unknown-linux-gnu"

    # Try AppImage via electron-builder (requires running on Linux)
    if [[ "$(uname -s)" == "Linux" ]]; then
        cd "$ELECTRON_DIR" && npm install && npm run dist:linux:x64
        echo "    ✓ AppImage: $DIST_DIR/"
    else
        # Fallback: tarball
        local staging="$DIST_DIR/linux-x64"
        rm -rf "$staging" && mkdir -p "$staging"
        cp "$PROJECT_ROOT/target/x86_64-unknown-linux-gnu/release/knotcoind" "$staging/"
        cp -r "$PROJECT_ROOT/share/explorer" "$staging/"
        tar -czf "$DIST_DIR/Knotcoin-$VERSION-linux-x64.tar.gz" -C "$staging" .
        echo "    ✓ Tarball: $DIST_DIR/Knotcoin-$VERSION-linux-x64.tar.gz"
        echo "    ℹ For AppImage, run this script on a Linux machine."
    fi
}

build_windows() {
    echo "=== Windows x86_64 ==="
    build_rust "x86_64-pc-windows-gnu" "knotcoind.exe"
    cp "$PROJECT_ROOT/target/x86_64-pc-windows-gnu/release/knotcoind.exe" \
       "$BINARIES_DIR/knotcoind-x86_64-pc-windows-msvc.exe"

    # Try NSIS via electron-builder (requires running on Windows or Wine)
    if [[ "$(uname -s)" == MINGW* ]] || [[ "$(uname -s)" == MSYS* ]]; then
        cd "$ELECTRON_DIR" && npm install && npm run dist:win:x64
        echo "    ✓ NSIS: $DIST_DIR/"
    else
        # Fallback: zip
        local staging="$DIST_DIR/win-x64"
        rm -rf "$staging" && mkdir -p "$staging"
        cp "$PROJECT_ROOT/target/x86_64-pc-windows-gnu/release/knotcoind.exe" "$staging/"
        cp -r "$PROJECT_ROOT/share/explorer" "$staging/"
        cd "$DIST_DIR" && zip -r "Knotcoin-$VERSION-win-x64.zip" win-x64/
        echo "    ✓ ZIP: $DIST_DIR/Knotcoin-$VERSION-win-x64.zip"
        echo "    ℹ For NSIS installer, run this script on Windows."
    fi
}

case "${1:-all}" in
    macos)   build_macos ;;
    linux)   build_linux ;;
    windows) build_windows ;;
    all)
        build_macos
        build_linux
        build_windows
        echo ""
        echo "=== Build Summary ==="
        ls -lh "$DIST_DIR"/*.dmg "$DIST_DIR"/*.tar.gz "$DIST_DIR"/*.zip 2>/dev/null || true
        ;;
    *)
        echo "Usage: $0 [macos|linux|windows|all]"
        exit 1
        ;;
esac
