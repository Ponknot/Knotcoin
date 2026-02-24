# Changelog

All notable changes to Knotcoin are documented in this file.

---

## [1.0.0] - 2026-02-24

### Genesis Release

**Status:** Production Ready  
**Security Rating:** A+ (Excellent)

### Core Features

#### Quantum-Resistant Cryptography
- Dilithium3 signatures (NIST FIPS 204, Security Level 3)
- SHA-512 for address derivation (256-bit quantum security)
- SHA3-256 for proof-of-work (128-bit quantum security)
- Argon2id + AES-256-GCM for wallet encryption

#### Memory-Hard Proof-of-Work (PONC)
- 2 MB scratchpad (ASIC-resistant)
- 512 mixing rounds
- Personalized per miner (prevents pool optimization)
- Fair mining on consumer hardware

#### Blockchain
- 60-second block time
- 50 KB minimum block size, 500 KB maximum
- Difficulty adjustment every 60 blocks (~1 hour)
- Transaction size: ~5.4 KB (Dilithium3 signatures)

#### Three-Phase Emission
- Phase 1 (6 months): 0.1 → 1.0 KOT linear ramp
- Phase 2 (6 months): 1.0 KOT constant
- Phase 3 (forever): 1.0 / log₂(x+2) decay

#### Referral System
- 5% bonus (protocol-minted, not deducted from miner)
- Single-hop only (no multi-level marketing)
- Active mining required (48-hour window)
- Governance weight from referrals

#### Governance
- Vote tracking and tallying
- Weight: 100 + 100 × log₁₀(contributions)
- 10% hard cap per entity (prevents centralization)
- On-chain vote deduplication

#### Wallet
- 24-word mnemonic generation (256-bit entropy)
- Encrypted keystore (Argon2id + AES-256-GCM)
- Deterministic key derivation (BIP-39 compatible)
- KOT1 address format with checksum

#### Network
- P2P protocol (port 9000)
- JSON-RPC API (port 9001)
- Peer discovery via ADDR gossip
- No hardcoded bootstrap peers (decentralized)

#### Explorer
- Web-based blockchain explorer
- Transaction viewer
- Governance dashboard
- Referral system interface

### Security

- All 45 unit tests passing
- Zero compiler errors
- Zero clippy warnings
- Comprehensive input validation
- Integer overflow protection
- No unsafe code (except justified FFI)
- Attack vectors tested and mitigated

### Immutable Rules

These cannot be changed:
- Dilithium3 signatures
- SHA-512/SHA3-256 hashing
- PONC algorithm (2 MB, 512 rounds)
- Emission schedule (all three phases)
- Referral structure (5%, single-hop, 48-hour window)
- Block parameters (60s time, 50 KB min size, 1 knot min fee)
- Fair launch (no pre-mine, no ICO, no admin allocation)

### Tunable Parameters

Can be changed via governance:
- Block size ceiling (50-500 KB)
- PONC scratchpad size (2-256 MB)
- State channel parameters
- Recommended fees
- Connection limits

**Note:** Governance enforcement is Phase 1 (vote tracking only). Parameter changes can be implemented in future versions.

### Known Limitations

- Governance tracks votes but doesn't enforce parameter changes (Phase 1)
- Layer 2 not implemented (documented as "Future Work")
- RPC has no authentication (bind to localhost recommended)
- Privacy is pseudonymous by design (use VPN/Tor for anonymity)

### Platforms

- macOS ARM64 (Apple Silicon)
- macOS Intel (x86_64)
- Linux (x86_64, ARM64)
- Windows (x86_64)

---

## [Unreleased]

### Planned for Phase 2
- Bilateral state channels (Layer 2 scaling)
- Governance parameter enforcement
- HTLC routing
- Multi-hop payment network

### Community Contributions Welcome
- Privacy enhancements
- Light client protocol
- Mobile wallets
- Additional tooling

---

[1.0.0]: https://github.com/Ponknot/Knotcoin/releases/tag/v1.0.0
