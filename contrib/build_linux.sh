#!/usr/bin/env bash
# build_linux.sh  builds knotcoind + knotcoin-cli for Linux x86_64
# Requires: rustup (stable >= 1.88), gcc/g++ (>= 10), cmake
set -euo pipefail

BOLD='\033[1m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
info()  { echo -e "${BOLD}[build]${NC} $*"; }
ok()    { echo -e "${GREEN}[ok]${NC} $*"; }
die()   { echo -e "${RED}[err]${NC} $*"; exit 1; }

# SCRIPT_DIR is the directory containing this script (contrib/)
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# PROJ_ROOT is the parent directory (KNOTCOIN/)
PROJ_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

OUT="$PROJ_ROOT/dist/linux-x86_64"
mkdir -p "$OUT"

info "Checking toolchain"
command -v cargo  >/dev/null || die "cargo not found  install rustup: https://rustup.rs"
command -v g++    >/dev/null || die "g++ not found  apt install build-essential"
command -v cc     >/dev/null || die "cc not found"

RUST_VER=$(rustc --version | awk '{print $2}')
info "Rust $RUST_VER"

NEED="1.88.0"
if printf '%s\n%s\n' "$NEED" "$RUST_VER" | sort -V -C; then
    ok "Rust version OK"
else
    die "Need Rust >= $NEED (have $RUST_VER). Run: rustup update stable"
fi

info "Building release binaries"
cd "$PROJ_ROOT"

RUSTFLAGS="-C target-cpu=x86-64" \
cargo build \
    --release \
    --bin knotcoind \
    --bin knotcoin-cli

ok "Compilation done"

info "Stripping and packaging"
BINS=(target/release/knotcoind target/release/knotcoin-cli)
for bin_path in "${BINS[@]}"; do
    full_path="$PROJ_ROOT/$bin_path"
    [ -f "$full_path" ] || die "Missing $full_path  did the build succeed?"
    strip "$full_path"
    cp "$full_path" "$OUT/"
    SIZE=$(du -sh "$OUT/$(basename "$full_path")" | cut -f1)
    ok "$(basename "$full_path")  $OUT/ ($SIZE)"
done

ARCHIVE="$PROJ_ROOT/dist/knotcoin-mainnet-linux-x86_64.tar.gz"
tar -czf "$ARCHIVE" \
    -C "$OUT" knotcoind knotcoin-cli \
    -C "$PROJ_ROOT" share/explorer \
    -C "$PROJ_ROOT" README.md 2>/dev/null || true

ok "Archive  $ARCHIVE"
sha256sum "$ARCHIVE" > "$ARCHIVE.sha256"
cat "$ARCHIVE.sha256"

echo ""
echo -e "${BOLD}Done.${NC}"
echo "  Run daemon:  ./knotcoind"
echo "  CLI:         ./knotcoin-cli getblockcount"
echo "  Explorer:    open share/explorer/index.html in your browser"
