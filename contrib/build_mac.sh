#!/usr/bin/env bash
# build_mac.sh  builds knotcoind + knotcoin-cli for macOS Apple Silicon (arm64)
# Requires: Xcode Command Line Tools, rustup (stable >= 1.88)
set -euo pipefail

BOLD='\033[1m'; GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
info()  { echo -e "${BOLD}[build]${NC} $*"; }
ok()    { echo -e "${GREEN}[ok]${NC} $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC} $*"; }
die()   { echo -e "${RED}[err]${NC} $*"; exit 1; }

# ROOT is the directory containing this script (contrib/)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# PROJ_ROOT is the parent directory (KNOTCOIN/)
PROJ_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

ARCH=$(uname -m)
OS=$(uname -s)

[ "$OS" = "Darwin" ] || die "This script is for macOS only. Use build_linux.sh on Linux."

if [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-apple-darwin"
    OUT="$PROJ_ROOT/dist/macos-arm64"
    info "Building for Apple Silicon (arm64)"
elif [ "$ARCH" = "x86_64" ]; then
    TARGET="x86_64-apple-darwin"
    OUT="$PROJ_ROOT/dist/macos-x86_64"
    warn "Building for Intel Mac (x86_64). For Apple Silicon use an M-series Mac."
else
    die "Unknown architecture: $ARCH"
fi

mkdir -p "$OUT"

info "Checking toolchain"
if ! xcode-select -p >/dev/null 2>&1; then
    die "Xcode Command Line Tools not installed. Run: xcode-select --install"
fi
ok "Xcode CLT: $(xcode-select -p)"

command -v cargo >/dev/null || die "cargo not found  install rustup: https://rustup.rs"

RUST_VER=$(rustc --version | awk '{print $2}')
info "Rust $RUST_VER"

NEED="1.88.0"
if printf '%s\n%s\n' "$NEED" "$RUST_VER" | sort -V -C; then
    ok "Rust version OK"
else
    die "Need Rust >= $NEED (have $RUST_VER). Run: rustup update stable"
fi

rustup target add "$TARGET" 2>/dev/null || true

if [ "$ARCH" = "arm64" ]; then
    export CXXFLAGS="-arch arm64 -mmacosx-version-min=12.0"
    export CFLAGS="$CXXFLAGS"
    export LDFLAGS="-arch arm64"
fi

info "Building release binaries for $TARGET"
cd "$PROJ_ROOT"

cargo build \
    --release \
    --target "$TARGET" \
    --bin knotcoind \
    --bin knotcoin-cli

ok "Compilation done"

info "Stripping and packaging"
BINS=(
    "target/$TARGET/release/knotcoind"
    "target/$TARGET/release/knotcoin-cli"
)

for bin_path in "${BINS[@]}"; do
    full_path="$PROJ_ROOT/$bin_path"
    [ -f "$full_path" ] || die "Missing $full_path"
    strip "$full_path"
    cp "$full_path" "$OUT/"
    SIZE=$(du -sh "$OUT/$(basename "$full_path")" | cut -f1)
    ok "$(basename "$full_path")  $OUT/ ($SIZE)"
done

info "Ad-hoc code signing"
for bin in "$OUT/knotcoind" "$OUT/knotcoin-cli"; do
    codesign --force --sign - "$bin" 2>/dev/null && ok "Signed $(basename "$bin")" || warn "codesign failed (non-fatal)"
done

ARCHIVE="$PROJ_ROOT/dist/knotcoin-mainnet-macos-$(echo "$ARCH" | sed 's/arm64/arm64/;s/x86_64/intel/').tar.gz"
tar -czf "$ARCHIVE" \
    -C "$OUT" knotcoind knotcoin-cli \
    -C "$PROJ_ROOT" share/explorer \
    -C "$PROJ_ROOT" README.md 2>/dev/null || true

ok "Archive  $ARCHIVE"
shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"
cat "$ARCHIVE.sha256"

echo ""
echo -e "${BOLD}Done.${NC}"
echo "  Run daemon:  ./knotcoind"
echo "  CLI:         ./knotcoin-cli getblockcount"
echo "  Explorer:    open share/explorer/index.html"
echo ""
