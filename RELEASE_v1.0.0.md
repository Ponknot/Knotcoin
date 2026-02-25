# Knotcoin v1.0.0 - Production Release

**Release Date**: February 24, 2026  
**Status**: Ready for Genesis Block Deployment  

---

## Overview

Knotcoin is the first proof-of-work blockchain with a protocol-level referral system. It combines memory-hard mining, quantum-resistant signatures, and network-driven growth incentives.

### Key Features

- **Memory-Hard Mining**: PONC algorithm with 2 MB scratchpad prevents ASIC dominance
- **Quantum-Resistant**: Dilithium3 signatures (NIST FIPS 204) protect against quantum attacks
- **Fair Launch**: No pre-mine, no ICO, no founder allocation
- **Referral System**: 5% bonus for network growth, built into consensus
- **Tunable Governance**: Adjustable parameters without hard forks

---

## Downloads

### Pre-built Binaries

| Platform | Binary | Size | SHA256 Checksum |
|----------|--------|------|-----------------|
| Linux x86_64 | [knotcoind-linux-x86_64](https://github.com/Ponknot/Knotcoin/releases/download/v1.0.0/knotcoind-linux-x86_64) | 3.1 MB | `1a895f0be6a352d6b93c1b67d2f85a6d2d38cd67cc372ff2d25d2cfd7e407643` |
| macOS Intel | [knotcoind-macos-intel](https://github.com/Ponknot/Knotcoin/releases/download/v1.0.0/knotcoind-macos-intel) | 2.1 MB | `12014ed16ef7baad13ac1ce0009cb174be0faca74dfb98c6ae3ede46650a7c60` |
| macOS ARM64 | [knotcoind-macos-arm64](https://github.com/Ponknot/Knotcoin/releases/download/v1.0.0/knotcoind-macos-arm64) | 1.9 MB | `ead0db122b7544d84e83595901959c51b15348a3bd9e8c1375d8e69270bb26e1` |
| Windows x86_64 | [knotcoind-windows-x86_64.exe](https://github.com/Ponknot/Knotcoin/releases/download/v1.0.0/knotcoind-windows-x86_64.exe) | 2.4 MB | `40b056b7d4bf9d3a16cc9d20e93d0eb813c85c6e5a2f2b1ec06e802514d3e7db` |

### Verify Downloads

**Linux/macOS:**
```bash
sha256sum knotcoind-linux-x86_64
# or
shasum -a 256 knotcoind-macos-arm64
```

**Windows:**
```powershell
certutil -hashfile knotcoind-windows-x86_64.exe SHA256
```

Compare the output with the checksums above. Do not run if they don't match.

---

## Quick Start

### 1. Download and Verify

Download the binary for your platform and verify the checksum (see table above).

### 2. Make Executable (Linux/macOS)

```bash
chmod +x knotcoind-*
```

### 3. Run the Node

```bash
./knotcoind-linux-x86_64
# or
./knotcoind-macos-arm64
# or
./knotcoind-macos-intel
```

**Windows:**
```cmd
knotcoind-windows-x86_64.exe
```

The node will:
- Create data directory at `~/.knotcoin/mainnet`
- Start P2P server on port 9000
- Start RPC server on port 9001 (localhost only)
- Begin syncing the blockchain

### 4. Open the Explorer

Navigate to `share/explorer/index.html` in your browser, or run:

```bash
cd share/explorer
python3 -m http.server 8080
```

Then visit: http://localhost:8080

---

## Building from Source

### Prerequisites

- Rust 1.70+ ([rustup.rs](https://rustup.rs))
- C++ compiler (for PONC algorithm)

**macOS:**
```bash
xcode-select --install
```

**Linux:**
```bash
sudo apt install build-essential cmake
```

**Windows:**
- Visual Studio 2022 with C++ tools
- Rust from rustup.rs

### Build

```bash
git clone https://github.com/Ponknot/Knotcoin.git
cd Knotcoin
cargo build --release
```

Binaries will be in `target/release/`

### Run Tests

```bash
cargo test --lib
```

All 76 tests should pass.

---

## Technical Specifications

### Consensus
- **Algorithm**: PONC (Proof of Network Confidence)
- **Scratchpad**: 2 MB memory-hard
- **Rounds**: 512 (tunable via governance)
- **Block Time**: 60 seconds
- **Difficulty Adjustment**: Every 60 blocks

### Cryptography
- **Signatures**: Dilithium3 (NIST FIPS 204)
- **PoW Hash**: SHA3-256 (NIST FIPS 202)
- **Address Hash**: SHA-512
- **Key Sizes**: PK=1952 bytes, Sig=3309 bytes

### Emission Schedule
- **Phase 1** (0-6 months): Linear ramp 0.1 → 1.0 KOT
- **Phase 2** (6-12 months): Constant 1.0 KOT
- **Phase 3** (Year 2+): Logarithmic decay to ~13M total supply

### Referral System
- **Bonus**: 5% of block reward (freshly minted)
- **Window**: 2,880 blocks (48 hours)
- **Structure**: Single-hop only
- **Threshold**: None (works for all reward sizes)

### Governance
- **Voting Weight**: Based on blocks mined (last year)
- **Cap**: 10% per entity (tunable 5-20%)
- **Threshold**: 51% for proposals
- **Activation**: 1,000 blocks after vote

---

## Network Ports

- **Port 9000**: P2P network (should be open for incoming)
- **Port 9001**: RPC API (localhost only, do not expose)

See README.md for firewall configuration instructions.

---

## Documentation

- **README.md**: Complete user guide
- **INSTALL.md**: Build instructions
- **GOVERNANCE.md**: Governance system details
- **TOKENOMICS.md**: Economic model
- **Whitepaper**: `share/explorer/whitepaper.html`

---

## What's Immutable

These rules cannot be changed after genesis:

- Dilithium3 signatures
- SHA-512 for addresses, SHA3-256 for PoW
- PONC algorithm structure (2 MB scratchpad)
- Emission schedule (all three phases)
- Referral structure (5%, single-hop, no threshold)
- Block time (60 seconds)
- Minimum fee (1 knot)
- No pre-mine

---

## What's Tunable

These parameters can be adjusted via governance vote:

- Governance cap (5-20%, default 10%)
- PONC rounds (256-2048, default 512)
- Block size ceiling (50-500 KB)
- State channel parameters

---

## Security

### Code Quality
- 76 comprehensive tests (all passing)
- 5,720 lines of Rust and C++ code
- Memory-safe implementation
- Security-focused design

### Cryptographic Primitives
- Dilithium3: Post-quantum signature scheme
- SHA3-256: NIST-standardized PoW hash
- SHA-512: Address derivation
- AES-256-GCM: Wallet encryption

---

## Support

- **GitHub Issues**: https://github.com/Ponknot/Knotcoin/issues
- **Documentation**: See README.md and docs in repository

---

## License

MIT License - See LICENSE file

---

## Verification

This release has been:
- Fully tested (76/76 tests passing)
- Security reviewed
- Cross-platform tested
- Ready for production deployment

**Commit**: 943880a  
**Repository**: https://github.com/Ponknot/Knotcoin  

---

*Built to outlast its creator.*
