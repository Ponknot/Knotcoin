#!/usr/bin/env bash
# install.sh — Knotcoin Mainnet installer
# Usage: curl -fsSL https://raw.githubusercontent.com/.../install.sh | bash
set -euo pipefail

REPO="codetohunt/knotcoin"
TAG="${KNOTCOIN_VERSION:-latest}"

BOLD='\033[1m'; GREEN='\033[0;32m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'
info() { echo -e "${BOLD}[knotcoin]${NC} $*"; }
ok()   { echo -e "${GREEN}  ✓${NC} $*"; }
die()  { echo -e "${RED}  ✗ Error:${NC} $*"; exit 1; }

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║      Knotcoin Mainnet Installer               ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════╝${NC}"
echo ""

OS=$(uname -s)
ARCH=$(uname -m)
INSTALL_DIR="${KNOTCOIN_DIR:-$HOME/.knotcoin}"

case "$OS-$ARCH" in
    Linux-x86_64)   PLATFORM="linux-x86_64" ;;
    Darwin-arm64)   PLATFORM="macos-arm64"  ;;
    Darwin-x86_64)  PLATFORM="macos-x86_64" ;;
    *) die "Unsupported platform: $OS $ARCH" ;;
esac

info "Platform: $PLATFORM"

# Resolve latest tag if not pinned
if [ "$TAG" = "latest" ]; then
    info "Fetching latest release tag…"
    TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": *"\(.*\)".*/\1/')
    [ -n "$TAG" ] || die "Could not resolve latest release. Set KNOTCOIN_VERSION=v1.0.0 manually."
fi

info "Installing $TAG"
ARCHIVE="knotcoin-mainnet-${PLATFORM}.tar.gz"
URL="https://github.com/$REPO/releases/download/$TAG/$ARCHIVE"
SHA_URL="${URL}.sha256"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

info "Downloading…"
curl -fsSL --progress-bar "$URL"     -o "$TMP/$ARCHIVE"
curl -fsSL              "$SHA_URL"   -o "$TMP/$ARCHIVE.sha256"
ok "Downloaded"

info "Verifying SHA-256…"
(cd "$TMP" && sha256sum -c "$ARCHIVE.sha256" 2>/dev/null) || \
(cd "$TMP" && shasum -a 256 -c "$ARCHIVE.sha256" 2>/dev/null) || \
die "Checksum mismatch — download may be corrupt"
ok "Checksum verified"

info "Installing to $INSTALL_DIR…"
mkdir -p "$INSTALL_DIR/bin" "$INSTALL_DIR/explorer"
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"
cp "$TMP/knotcoind"    "$INSTALL_DIR/bin/"
cp "$TMP/knotcoin-cli" "$INSTALL_DIR/bin/"
chmod +x "$INSTALL_DIR/bin/knotcoind" "$INSTALL_DIR/bin/knotcoin-cli"
[ -d "$TMP/explorer" ] && cp -r "$TMP/explorer/" "$INSTALL_DIR/explorer/"

# Remove macOS quarantine
if [ "$OS" = "Darwin" ]; then
    xattr -dr com.apple.quarantine "$INSTALL_DIR/bin/knotcoind"  2>/dev/null || true
    xattr -dr com.apple.quarantine "$INSTALL_DIR/bin/knotcoin-cli" 2>/dev/null || true
fi
ok "Binaries installed"

# Shell PATH setup
SHELL_RC=""
case "$SHELL" in
    */zsh)  SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
esac

if [ -n "$SHELL_RC" ]; then
    EXPORT_LINE='export PATH="$HOME/.knotcoin/bin:$PATH"'
    if ! grep -qF ".knotcoin/bin" "$SHELL_RC" 2>/dev/null; then
        echo "" >> "$SHELL_RC"
        echo "# Knotcoin" >> "$SHELL_RC"
        echo "$EXPORT_LINE" >> "$SHELL_RC"
        ok "Added to $SHELL_RC"
    fi
fi

echo ""
echo -e "${GREEN}${BOLD}Installation complete.${NC}"
echo ""
echo "  Start node:   $INSTALL_DIR/bin/knotcoind"
echo "  CLI:          $INSTALL_DIR/bin/knotcoin-cli getblockcount"
echo "  Explorer:     open $INSTALL_DIR/explorer/index.html"
echo ""
echo "  Or reload your shell and use directly:"
echo "    source $SHELL_RC"
echo "    knotcoind"
echo ""
