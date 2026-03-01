# Knotcoin

A quantum-resistant blockchain with post-quantum cryptography (Dilithium3), referral rewards, and decentralized governance.

## Features

- Post-quantum signatures (NIST FIPS 204 Dilithium3)
- Proof-of-Work consensus with dynamic difficulty
- Referral system with 5% bonus rewards
- Decentralized governance voting
- Real-time network visualization
- Cross-platform desktop wallet

## Quick Start

### Download

Get the latest release for your platform:
- macOS (Apple Silicon): `Knotcoin-1.0.2-arm64.dmg`
- Windows (x64): `Knotcoin-1.0.2-win-x64.zip`
- Linux (x64): `Knotcoin-1.0.2-linux-x64.tar.gz`

### Installation

**macOS:**
```bash
# Open DMG and drag to Applications
open Knotcoin-1.0.2-arm64.dmg
```

**Windows:**
```bash
# Extract ZIP and run
Knotcoin.bat
```

**Linux:**
```bash
# Extract and run
tar xzf Knotcoin-1.0.2-linux-x64.tar.gz
cd Knotcoin-linux-x64
./knotcoin.sh
```

### First Use

1. Launch the application
2. Open browser to `http://localhost:19001`
3. Create wallet with 24-word mnemonic
4. Start mining to earn rewards

## Network Information

### Bootstrap Nodes

The network uses P2P peer discovery with DNS-based bootstrap:
- DNS: `seed.knotcoin.network:9000`

You can also specify custom bootstrap peers via environment variable:
```bash
export KNOTCOIN_BOOTSTRAP_PEERS="peer1.example.com:9000,peer2.example.com:9000"
```

### Ports

- P2P: 9000 (must be open for incoming connections)
- RPC: 9001 (localhost only)
- Web UI: 19001 (localhost only)

### Data Storage

**Windows:** `C:\Users\<Username>\.knotcoin\mainnet\`  
**macOS:** `/Users/<Username>/.knotcoin/mainnet/`  
**Linux:** `/home/<Username>/.knotcoin/mainnet/`

Files stored:
- `blockchain.db/` - Blockchain data (RocksDB)
- `wallet.dat` - Encrypted wallet (AES-256-GCM)
- `peers.json` - Known peer addresses
- `.cookie` - RPC authentication token

## Building from Source

### Prerequisites

- Rust 1.75+ ([install rustup](https://rustup.rs/))
- Cargo (included with Rust)

### Build

```bash
# Clone repository
git clone https://github.com/Ponknot/Knotcoin.git
cd Knotcoin

# Build release binaries
cargo build --release

# Binaries in target/release/
# - knotcoind (node + miner)
# - knotcoin-cli (command-line interface)
```

### Package

```bash
# Create distribution packages
./package_native.sh all

# Packages created in dist/
# - Knotcoin-1.0.2-arm64.dmg (macOS)
# - Knotcoin-1.0.2-win-x64.zip (Windows)
# - Knotcoin-1.0.2-linux-x64.tar.gz (Linux)
```

## Architecture

### Cryptography

- **Signatures:** Dilithium3 (ML-DSA-65, NIST FIPS 204)
- **Hashing:** SHA-512, SHA3-256, BLAKE3
- **Key Derivation:** BIP-39 mnemonics, PBKDF2-HMAC-SHA512
- **Wallet Encryption:** AES-256-GCM, Argon2

### Consensus

- **Algorithm:** Proof-of-Work (SHA-512 mining)
- **Block Time:** ~60 seconds target
- **Difficulty:** Adjusts every 10 blocks
- **Block Reward:** 50 KOT (halves every 210,000 blocks)

### Referral System

- Register referrer via zero-amount transaction
- Referrer earns 5% bonus on miner's block rewards
- Bonus decays over time (halves every 10,000 blocks)

### Governance

- On-chain proposal voting
- Proposals stored in transaction data
- Vote weight based on balance

## RPC API

### Wallet

```bash
# Create wallet from mnemonic
knotcoin-cli wallet_create_file "<24-word-mnemonic>" "<password>"

# Get address
knotcoin-cli wallet_get_address

# Get balance
knotcoin-cli wallet_get_balance

# Send transaction
knotcoin-cli wallet_send "<recipient>" <amount>

# Register referrer
knotcoin-cli wallet_register_referral "<referrer-address>"
```

### Mining

```bash
# Start mining
knotcoin-cli miner_start

# Stop mining
knotcoin-cli miner_stop

# Get mining status
knotcoin-cli miner_status
```

### Blockchain

```bash
# Get blockchain info
knotcoin-cli getblockchaininfo

# Get block by height
knotcoin-cli getblockbyheight <height>

# Get transaction
knotcoin-cli gettransaction <txid>
```

### Network

```bash
# Get peer info
knotcoin-cli getpeerinfo

# Get network miners
knotcoin-cli getnetworkminers
```

## Security

### Wallet Security

- Mnemonic never stored on disk (only hint)
- Secret key encrypted with AES-256-GCM
- Password hashed with Argon2 (memory-hard)
- Deterministic key generation (same mnemonic = same address)

### Network Security

- RPC authentication via cookie file
- RPC bound to localhost only
- P2P message size limits
- Transaction validation before propagation

### Quantum Resistance

Dilithium3 provides NIST Security Level 3:
- Equivalent to AES-192 against quantum computers
- Based on lattice cryptography (Module-LWE)
- Standardized in NIST FIPS 204 (2024)

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

### Development

```bash
# Run tests
cargo test

# Run specific test
cargo test test_name

# Check code
cargo clippy

# Format code
cargo fmt
```

## License

See [LICENSE](LICENSE) file.

## Support

- GitHub Issues: https://github.com/Ponknot/Knotcoin/issues
- Documentation: See `docs/` directory

## Acknowledgments

- NIST for Dilithium standardization
- Rust cryptography community
- Early network participants
