# Knotcoin

A quantum-resistant blockchain with post-quantum cryptography (Dilithium3), referral rewards, and decentralized governance.

## Features

- Post-quantum signatures (NIST FIPS 204 Dilithium3)
- Proof-of-Work consensus with dynamic difficulty
- Referral system with 5% bonus rewards
- Decentralized governance voting
- Real-time network visualization
- Cross-platform desktop wallet

## Quick Start (v1.0.3)

### Download

Get the latest release for your platform:
- macOS (Apple Silicon): `Knotcoin-1.0.3-arm64.dmg` (SHA-256: `c02b56630b6c6fd1769a44d3ba5694ed24399e900fc087e822d1cc158898f703`)
- Windows (x64): `Knotcoin-1.0.3-Windows-x64.zip`
- Linux (x64): `Knotcoin-1.0.3-Linux-x64.tar.gz`

### Installation

**macOS:**
```bash
# Open DMG and drag to Applications
open Knotcoin-1.0.3-macOS-arm64.dmg
```

**Windows:**
```bash
# Extract ZIP and run
Knotcoin.bat
```

**Linux:**
```bash
# Extract and run
tar xzf Knotcoin-1.0.3-Linux-x64.tar.gz
cd Knotcoin-Linux
./knotcoin.sh
```

### First Use

1. Launch the application
2. The UI opens automatically in your browser (local HTML)
3. If prompted for RPC auth, paste the token from `~/.knotcoin/mainnet/.cookie`
4. Create a wallet with a 24-word mnemonic
5. Start mining to earn rewards

## Network Information

### Bootstrap Nodes

The network uses P2P peer discovery with bootstrap seeds:
- DNS: `seed.knotcoin.network:9000`
- Volunteer bootstrap IP: `104.229.254.145:9000`

You can also specify custom bootstrap peers via environment variable:
```bash
export KNOTCOIN_BOOTSTRAP_PEERS="peer1.example.com:9000,peer2.example.com:9000"
```

You can add more community seeds in:
- `~/.knotcoin/mainnet/seedlist.txt` (one host:port per line)

### Ports

- P2P: 9000 (must be open for incoming connections)
- RPC: 9001 (localhost only)
- Web UI: Local browser file (opened automatically; uses RPC token from `.cookie`)

### Data Storage

**Windows:** `C:\Users\<Username>\.knotcoin\mainnet\`  
**macOS:** `/Users/<Username>/.knotcoin/mainnet/`  
**Linux:** `/home/<Username>/.knotcoin/mainnet/`

Files stored:
- `blockchain.db/` - Blockchain data (RocksDB)
- `wallet_keys.json` - Wallet keys cache (local profile)
- `seedlist.txt` - Optional community seed list
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
./build_all_packages.sh

# Packages created in dist/
# - Knotcoin-1.0.3-macOS-arm64.dmg (macOS)
# - Knotcoin-1.0.3-Windows-x64.zip (Windows)
# - Knotcoin-1.0.3-Linux-x64.tar.gz (Linux)
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

- Mnemonic not stored on disk (only a hint)
- RPC cookie auth enabled by default
- Keep your `.cookie` and wallet files private

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

## Troubleshooting

- **CGNAT / Port 9000 closed:** You can still sync and mine, but you will not be reachable as a public node. Use community seeds for bootstrap.
- **No peers:** Confirm your internet connection and that seed nodes are reachable. Add more seeds to `seedlist.txt`.
- **UI says Disconnected:** Ensure `knotcoind` is running and listening on `127.0.0.1:9001`. Check `.cookie` under the data dir and refresh the page.
- **No realtime updates:** Keep the node running; the UI refreshes every 2 seconds from RPC.

## Realtime Sync Check

Use these checks to confirm your node is connected to the network and syncing in realtime.

1. Check current block height:
```bash
knotcoin-cli getblockcount
```

2. Check peer connectivity:
```bash
knotcoin-cli getpeerinfo
```
Expected: `connected: true` and `peer_count > 0`.

3. Watch height increase over time:
```bash
watch -n 2 knotcoin-cli getblockcount
```
If `watch` is unavailable, run `knotcoin-cli getblockcount` every few seconds.

4. Verify network hashrate endpoint:
```bash
knotcoin-cli getnetworkhashrate
```

### Browser UI + RPC Auth

- The browser UI reads realtime data from local RPC at `127.0.0.1:9001`.
- If prompted for auth, use token from:
`~/.knotcoin/mainnet/.cookie`
- If you get `Unauthorized`, clear cached token in browser console:
```js
localStorage.removeItem('knotcoin_auth')
```
Then refresh and enter the current token.

## Acknowledgments

- NIST for Dilithium standardization
- Rust cryptography community
- Early network participants
