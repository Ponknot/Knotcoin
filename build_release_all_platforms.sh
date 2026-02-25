#!/bin/bash
# Knotcoin Multi-Platform Release Builder
# Builds for: macOS (Intel + Apple Silicon), Linux (x86_64), Windows (x86_64)

set -e

VERSION="1.0.0"
RELEASE_DATE=$(date +%Y%m%d)
RELEASE_DIR="dist/knotcoin-v${VERSION}-${RELEASE_DATE}"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║   KNOTCOIN MULTI-PLATFORM RELEASE BUILDER                  ║"
echo "║   Version: ${VERSION}                                         ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check prerequisites
echo "🔍 Checking prerequisites..."
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: cargo not found. Install Rust first."
    exit 1
fi

if ! command -v git &> /dev/null; then
    echo "❌ Error: git not found."
    exit 1
fi

echo "   ✅ Rust toolchain found"
echo "   ✅ Git found"
echo ""

# Verify we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -f "src/config.rs" ]; then
    echo "❌ Error: Must run from Knotcoin root directory"
    exit 1
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo "⚠️  Warning: You have uncommitted changes"
    read -p "   Continue anyway? (y/N): " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo "Aborted. Commit your changes first."
        exit 0
    fi
fi

echo "📋 Build Configuration:"
echo "   Version: ${VERSION}"
echo "   Date: ${RELEASE_DATE}"
echo "   Output: ${RELEASE_DIR}"
echo ""

read -p "   Start multi-platform build? (y/N): " confirm
if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
    echo "Aborted."
    exit 0
fi

# Create release directory structure
echo ""
echo "📁 Creating release directory structure..."
mkdir -p "${RELEASE_DIR}"
mkdir -p "${RELEASE_DIR}/macos-intel"
mkdir -p "${RELEASE_DIR}/macos-apple-silicon"
mkdir -p "${RELEASE_DIR}/linux-x86_64"
mkdir -p "${RELEASE_DIR}/windows-x86_64"
echo "   ✅ Directories created"

# Backup current config
echo ""
echo "💾 Backing up private config..."
cp src/config.rs src/config.rs.private_backup

# Enable public P2P for release
echo ""
echo "✏️  Enabling public P2P for release builds..."
sed -i.tmp 's/pub const P2P_BIND_ADDRESS: &str = "127.0.0.1";/pub const P2P_BIND_ADDRESS: \&str = "0.0.0.0";/' src/config.rs
rm -f src/config.rs.tmp
echo "   ✅ Public P2P enabled"

# Clean previous builds
echo ""
echo "🧹 Cleaning previous builds..."
cargo clean
echo "   ✅ Clean complete"

# Build for current platform (macOS)
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║   BUILDING FOR MACOS                                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

CURRENT_ARCH=$(uname -m)
if [ "$CURRENT_ARCH" = "arm64" ]; then
    echo "🍎 Building for macOS Apple Silicon (arm64)..."
    cargo build --release --bin knotcoind --bin knotcoin-cli
    
    cp target/release/knotcoind "${RELEASE_DIR}/macos-apple-silicon/"
    cp target/release/knotcoin-cli "${RELEASE_DIR}/macos-apple-silicon/"
    echo "   ✅ macOS Apple Silicon build complete"
    
    # Try to build for Intel if possible
    if rustup target list | grep -q "x86_64-apple-darwin (installed)"; then
        echo ""
        echo "🍎 Building for macOS Intel (x86_64)..."
        cargo build --release --target x86_64-apple-darwin --bin knotcoind --bin knotcoin-cli
        
        cp target/x86_64-apple-darwin/release/knotcoind "${RELEASE_DIR}/macos-intel/"
        cp target/x86_64-apple-darwin/release/knotcoin-cli "${RELEASE_DIR}/macos-intel/"
        echo "   ✅ macOS Intel build complete"
    else
        echo "   ⚠️  Intel target not installed, skipping"
        echo "   To build for Intel: rustup target add x86_64-apple-darwin"
    fi
else
    echo "🍎 Building for macOS Intel (x86_64)..."
    cargo build --release --bin knotcoind --bin knotcoin-cli
    
    cp target/release/knotcoind "${RELEASE_DIR}/macos-intel/"
    cp target/release/knotcoin-cli "${RELEASE_DIR}/macos-intel/"
    echo "   ✅ macOS Intel build complete"
    
    # Try to build for Apple Silicon if possible
    if rustup target list | grep -q "aarch64-apple-darwin (installed)"; then
        echo ""
        echo "🍎 Building for macOS Apple Silicon (arm64)..."
        cargo build --release --target aarch64-apple-darwin --bin knotcoind --bin knotcoin-cli
        
        cp target/aarch64-apple-darwin/release/knotcoind "${RELEASE_DIR}/macos-apple-silicon/"
        cp target/aarch64-apple-darwin/release/knotcoin-cli "${RELEASE_DIR}/macos-apple-silicon/"
        echo "   ✅ macOS Apple Silicon build complete"
    else
        echo "   ⚠️  Apple Silicon target not installed, skipping"
        echo "   To build for Apple Silicon: rustup target add aarch64-apple-darwin"
    fi
fi

# Build for Linux (if cross-compilation is available)
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║   BUILDING FOR LINUX                                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

if rustup target list | grep -q "x86_64-unknown-linux-musl"; then
    # Add musl target if not installed
    if ! rustup target list | grep -q "x86_64-unknown-linux-musl (installed)"; then
        echo "   📦 Installing musl target..."
        rustup target add x86_64-unknown-linux-musl
    fi
    
    echo "🐧 Building for Linux x86_64 (musl)..."
    cargo build --release --target x86_64-unknown-linux-musl --bin knotcoind --bin knotcoin-cli
    
    cp target/x86_64-unknown-linux-musl/release/knotcoind "${RELEASE_DIR}/linux-x86_64/"
    cp target/x86_64-unknown-linux-musl/release/knotcoin-cli "${RELEASE_DIR}/linux-x86_64/"
    echo "   ✅ Linux x86_64 build complete"
else
    echo "   ⚠️  Linux musl target not available"
    echo "   Install: rustup target add x86_64-unknown-linux-musl"
    echo "   Linker: brew install filosottile/musl-cross/musl-cross"
fi

# Build for Windows (if cross-compilation is available)
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║   BUILDING FOR WINDOWS                                     ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

if rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
    echo "🪟 Building for Windows x86_64..."
    cargo build --release --target x86_64-pc-windows-gnu --bin knotcoind --bin knotcoin-cli
    
    cp target/x86_64-pc-windows-gnu/release/knotcoind.exe "${RELEASE_DIR}/windows-x86_64/" 2>/dev/null || true
    cp target/x86_64-pc-windows-gnu/release/knotcoin-cli.exe "${RELEASE_DIR}/windows-x86_64/" 2>/dev/null || true
    echo "   ✅ Windows x86_64 build complete"
else
    echo "   ⚠️  Windows target not installed, skipping"
    echo "   To build for Windows: rustup target add x86_64-pc-windows-gnu"
    echo "   Note: Requires MinGW-w64 toolchain"
fi

# Restore private config
echo ""
echo "🔄 Restoring private config..."
mv src/config.rs.private_backup src/config.rs
echo "   ✅ Private config restored"

# Copy essential documentation to each platform
echo ""
echo "📄 Copying documentation..."
for platform_dir in "${RELEASE_DIR}"/*/; do
    if [ -d "$platform_dir" ] && [ "$(ls -A $platform_dir)" ]; then
        cp README.md "$platform_dir/" 2>/dev/null || true
        cp LICENSE "$platform_dir/" 2>/dev/null || true
        cp TOKENOMICS.md "$platform_dir/" 2>/dev/null || true
        
        # Copy explorer
        cp -r share/explorer "$platform_dir/" 2>/dev/null || true
        
        # Create quick start guide
        cat > "$platform_dir/QUICKSTART.txt" << 'EOF'
KNOTCOIN QUICK START
====================

1. START NODE:
   ./knotcoind &

2. CREATE WALLET:
   ./knotcoin-cli createwallet
   (Save your mnemonic securely!)

3. START MINING:
   ./knotcoin-cli generatetoaddress 1 YOUR_KOT1_ADDRESS

4. CHECK BALANCE:
   ./knotcoin-cli getbalance YOUR_KOT1_ADDRESS

5. WEB EXPLORER (optional):
   cd explorer
   node server.js
   Open http://localhost:8080

PORTS:
- P2P: 9000 (must be open for incoming)
- RPC: 9001 (localhost only)

DOCUMENTATION:
- README.md - Full documentation
- TOKENOMICS.md - Economic model

SUPPORT:
- GitHub: [Your repository URL]
EOF
    fi
done
echo "   ✅ Documentation copied"

# Generate checksums for each platform
echo ""
echo "🔐 Generating checksums..."
for platform_dir in "${RELEASE_DIR}"/*/; do
    if [ -d "$platform_dir" ] && [ "$(ls -A $platform_dir)" ]; then
        platform_name=$(basename "$platform_dir")
        (
            cd "$platform_dir"
            if ls knotcoind* 1> /dev/null 2>&1; then
                shasum -a 256 knotcoind* knotcoin-cli* > SHA256SUMS 2>/dev/null || true
            fi
        )
        echo "   ✅ Checksums for $platform_name"
    fi
done

# Create archives for each platform
echo ""
echo "📦 Creating release archives..."
cd "${RELEASE_DIR}"
for platform_dir in */; do
    if [ -d "$platform_dir" ] && [ "$(ls -A $platform_dir)" ]; then
        platform_name=$(basename "$platform_dir")
        archive_name="knotcoin-v${VERSION}-${platform_name}.tar.gz"
        
        tar -czf "$archive_name" "$platform_dir"
        shasum -a 256 "$archive_name" >> ../RELEASE_CHECKSUMS.txt
        
        echo "   ✅ $archive_name"
    fi
done
cd - > /dev/null

# Summary
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║   MULTI-PLATFORM BUILD COMPLETE! 🎉                        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

echo "📊 Build Summary:"
echo ""
for platform_dir in "${RELEASE_DIR}"/*/; do
    if [ -d "$platform_dir" ] && [ "$(ls -A $platform_dir)" ]; then
        platform_name=$(basename "$platform_dir")
        echo "   ✅ $platform_name"
        if [ -f "$platform_dir/knotcoind" ] || [ -f "$platform_dir/knotcoind.exe" ]; then
            ls -lh "$platform_dir"/knotcoind* 2>/dev/null | awk '{print "      knotcoind:    " $5}'
        fi
        if [ -f "$platform_dir/knotcoin-cli" ] || [ -f "$platform_dir/knotcoin-cli.exe" ]; then
            ls -lh "$platform_dir"/knotcoin-cli* 2>/dev/null | awk '{print "      knotcoin-cli: " $5}'
        fi
        echo ""
    fi
done

echo "📦 Release Archives:"
ls -lh "${RELEASE_DIR}"/*.tar.gz 2>/dev/null | awk '{print "   " $9 " (" $5 ")"}'
echo ""

echo "🔐 Master Checksums:"
cat "${RELEASE_DIR}/../RELEASE_CHECKSUMS.txt"
echo ""

echo "📂 Release Location:"
echo "   ${RELEASE_DIR}"
echo ""

echo "🎯 Next Steps:"
echo "   1. Test each platform build"
echo "   2. Verify all checksums"
echo "   3. Create GitHub Release"
echo "   4. Upload archives + checksums"
echo "   5. Announce to community"
echo ""

echo "🔒 Your Node Status:"
echo "   ✅ Config restored to private (127.0.0.1)"
echo "   ✅ Your anonymity protected"
echo "   ✅ Release builds use public P2P (0.0.0.0)"
echo ""

echo "🎉 Ready for distribution!"
echo ""
