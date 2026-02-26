# Knotcoin

The first proof-of-work blockchain with protocol-level referral system. Memory-hard mining, quantum-resistant signatures, and network-driven growth.

---

## What is Knotcoin?

Knotcoin is the first cryptocurrency to combine proof-of-work consensus with a built-in referral system at the protocol level. It fixes three fundamental problems:

1. **Mining centralization**: Bitcoin mining is dominated by ASICs that cost millions. Knotcoin uses memory-hard proof-of-work (PONC) so regular computers can compete. The gap between a laptop and a datacenter is single-digit, not million-fold.

2. **Unfair launches**: Most coins have pre-mines, ICOs, or founder allocations. Knotcoin has none of that. The creator mines block zero under the same rules as everyone else. A 5% referral bonus rewards people who grow the network - built into consensus, not a smart contract.

3. **Quantum vulnerability**: Bitcoin's ECDSA signatures will break when quantum computers arrive. Knotcoin uses Dilithium3 (NIST FIPS 204), which is designed to survive quantum attacks.

**Unique Innovation:** First PoW blockchain with protocol-level referrals. No other cryptocurrency has this combination.

Deploy once, run forever. No foundation, no team, no updates required.

---

## Quick Start

### 1. Download & Extract

Get the latest release for your platform:

**macOS (Apple Silicon)**
```bash
wget https://github.com/Ponknot/Knotcoin/releases/download/v1.0.1/knotcoin-v1.0.1-macos-apple-silicon.tar.gz
tar -xzf knotcoin-v1.0.1-macos-apple-silicon.tar.gz
cd macos-apple-silicon
```

**macOS (Intel)**
```bash
wget https://github.com/Ponknot/Knotcoin/releases/download/v1.0.1/knotcoin-v1.0.1-macos-intel.tar.gz
tar -xzf knotcoin-v1.0.1-macos-intel.tar.gz
cd macos-intel
```

**Linux (x86_64)**
```bash
wget https://github.com/Ponknot/Knotcoin/releases/download/v1.0.1/knotcoin-v1.0.1-linux-x86_64.tar.gz
tar -xzf knotcoin-v1.0.1-linux-x86_64.tar.gz
cd linux-x86_64
```

**Windows (x86_64)**
```powershell
# Download: https://github.com/Ponknot/Knotcoin/releases/download/v1.0.1/knotcoin-v1.0.1-windows-x86_64.tar.gz
# Extract using 7-Zip or Windows built-in tar:
tar -xzf knotcoin-v1.0.1-windows-x86_64.tar.gz
cd windows-x86_64
```

### 2. Verify Checksums (Important!)

**macOS/Linux:**
```bash
shasum -a 256 knotcoin-v1.0.1-*.tar.gz
```

**Expected checksums:**
```
eb70aec56189244030ee8451b1ac18c629ae82503705d80ec03a95b42bb75360  knotcoin-v1.0.1-linux-x86_64.tar.gz
80237ae494882d9121ed9cad3e1ee04fb19bdb92538b35ffeeb0c3554ab0da4a  knotcoin-v1.0.1-macos-apple-silicon.tar.gz
bc59b551d457403048056018e05b0a83f1b25db56dcd2f93367362e3a3a585c8  knotcoin-v1.0.1-macos-intel.tar.gz
46118b5f9fa08a156583c7001e0769d0bcda381f2951b955029104748306392f  knotcoin-v1.0.1-windows-x86_64.tar.gz
```

**IMPORTANT**: Verify checksums match before running. Mismatched checksums mean corrupted or tampered files.

### 3. Run the Node

**macOS/Linux:**
```bash
chmod +x knotcoind knotcoin-cli
./knotcoind
```

**Windows:**
```cmd
knotcoind.exe
```

The node will:
- Create data directory at `~/.knotcoin/mainnet`
- Start syncing the blockchain
- Open RPC server on port 9001 (localhost only)
- Open P2P server on port 9000 (public)

### 4. Open the Explorer

The release includes a built-in web explorer. Open it in your browser:

**Option 1: Direct file access**
```
file:///path/to/knotcoin-v1.0.1-[platform]/explorer/index.html
```

**Option 2: Local web server (recommended)**
```bash
cd explorer
python3 -m http.server 8080
```

Then visit: `http://localhost:8080`

The explorer connects to your local node via RPC and shows:
- Live blockchain data
- Network visualization
- Wallet management
- Mining interface
- Block explorer

---

## Creating Your First Wallet

### Using the Web Explorer

1. Open the explorer in your browser
2. Click **WALLET** tab
3. Click **GENERATE MNEMONIC**
4. Write down your 24-word recovery phrase (NEVER share this!)
5. Click **I HAVE SAVED IT**

Your wallet is now active. The address starts with `KOT1`.

### Using the Command Line

```bash
# Generate a new wallet
./knotcoin-cli generatewallet

# This will output:
# Mnemonic: word1 word2 word3 ... word24
# Address: KOT1abc123...
```

**IMPORTANT:** Write down your 24-word mnemonic on paper. Store it safely. This is the ONLY way to recover your wallet.

---

## Mining Your First Block

### Using the Web Explorer

1. Go to **MINER** tab
2. Enter your wallet address (starts with KOT1)
3. Set blocks to mine (start with 1)
4. Click **START MINING**

Your computer will solve the proof-of-work puzzle. When successful, you'll see the block hash appear.

### Using the Command Line

```bash
# Mine 1 block to your address
./knotcoin-cli generatetoaddress 1 KOT1your_address_here

# Mine 10 blocks
./knotcoin-cli generatetoaddress 10 KOT1your_address_here
```

**First Block Reward:** 0.1 KOT (increases to 1.0 KOT over 6 months)

---

## Checking Your Balance

### Web Explorer
1. Go to **WALLET** tab
2. Your balance is displayed at the top

### Command Line
```bash
./knotcoin-cli getbalance KOT1your_address_here
```

Output:
```json
{
  "balance_kot": "1.50000000",
  "balance_knots": 150000000,
  "nonce": 0
}
```

---

## Sending a Transaction

### Command Line
```bash
./knotcoin-cli sendtoaddress \
  --from KOT1sender_address \
  --to KOT1recipient_address \
  --amount 0.5 \
  --fee 0.00001
```

**Note:** The web UI transaction sending is disabled until full Dilithium3 signing is integrated in-browser. Use the CLI for now.

---

## Understanding the Referral System

When you mine a block, your referrer gets a 5% bonus (freshly minted, not taken from your reward).

**How to set a referrer:**

1. Get a referral link: `knotcoin://[code]@host:port/node`
2. Extract the 8-byte code
3. Include it in your first transaction

**Rules:**
- One referrer per wallet (permanent)
- Referrer must have mined within last 48 hours
- Single-hop only (no pyramid structure)
- You can refer each other (mutual referrals work)

The referral link also serves as a bootstrap mechanism - it includes the referrer's node address, so clicking it connects you to your first peer. No central server needed.

---

## Network Ports

When you run `knotcoind`, it uses three ports:

**Port 9000 (P2P - Blockchain Network)**
- What: Connects you to other Knotcoin nodes
- Bound to: 0.0.0.0 (all network interfaces)
- Firewall: Should be OPEN for incoming connections
- Why: Lets other nodes discover and connect to you
- If blocked: You can still sync, but you won't help the network

**Port 9001 (RPC - Your Wallet API)**
- What: Local API for your wallet and mining commands
- Bound to: 127.0.0.1 (localhost only)
- Firewall: Already secure (localhost-only)
- Why: Keeps your wallet safe from internet access
- Never expose this to the internet!

**Port 8080 (Web Explorer - Optional)**
- What: Web interface for viewing blockchain
- Only if you run: `python3 -m http.server 8080`
- Not needed if you use the CLI

**For most users:** Just open port 9000 in your router/firewall. Port 9001 is already secure.

---

## Opening Port 9000 (Firewall Setup)

Opening port 9000 lets other nodes connect to you. This helps the network but isn't required - you can still use Knotcoin without it.

### Windows Firewall

1. Open Windows Defender Firewall
2. Click "Advanced settings"
3. Click "Inbound Rules" → "New Rule"
4. Choose "Port" → Next
5. Choose "TCP" → Type "9000" → Next
6. Choose "Allow the connection" → Next
7. Check all boxes (Domain, Private, Public) → Next
8. Name it "Knotcoin P2P" → Finish

### macOS Firewall

macOS firewall usually allows outgoing connections by default. If you have it enabled:

1. System Preferences → Security & Privacy → Firewall
2. Click "Firewall Options"
3. Click "+" and add `knotcoind`
4. Set to "Allow incoming connections"

Or use Terminal:
```bash
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /path/to/knotcoind
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp /path/to/knotcoind
```

### Linux (UFW)

```bash
sudo ufw allow 9000/tcp
sudo ufw status
```

### Linux (iptables)

```bash
sudo iptables -A INPUT -p tcp --dport 9000 -j ACCEPT
sudo iptables-save > /etc/iptables/rules.v4
```

### Router Port Forwarding

If you're behind a router (most home users are):

1. Find your computer's local IP:
   - Windows: `ipconfig` (look for IPv4 Address)
   - Mac/Linux: `ifconfig` or `ip addr` (look for 192.168.x.x)

2. Log into your router (usually http://192.168.1.1 or http://192.168.0.1)

3. Find "Port Forwarding" section (might be under Advanced or NAT)

4. Add new rule:
   - External Port: 9000
   - Internal Port: 9000
   - Internal IP: Your computer's IP (from step 1)
   - Protocol: TCP

5. Save and reboot router

**Note:** Every router is different. Google "[your router model] port forwarding" for specific instructions.

### Check If It Worked

After opening the port, check if it's accessible:

```bash
# On another computer or use a website like canyouseeme.org
telnet your_public_ip 9000
```

If it connects, you're good. If it times out, the port is still blocked.

---

## Data Directory

All blockchain data is stored in:
```
~/.knotcoin/mainnet/
```

**Contents:**
- `blocks/` - Block database
- `accounts/` - Account balances
- `mempool/` - Pending transactions
- `governance/` - Voting records

**Backup:** Copy this entire directory to backup your node data (not your wallet - use your 24-word mnemonic for that).

---

## Tokenomics

### Emission Schedule

**Phase 1 (Months 0-6):**
- Reward increases from 0.1 to 1.0 KOT
- Total minted: ~144,540 KOT

**Phase 2 (Months 6-12):**
- Constant 1.0 KOT per block
- Total minted: 262,800 KOT

**Phase 3 (Year 2+):**
- Logarithmic decay: `1.0 / log₂(blocks + 2)`
- Asymptotic approach to ~13M KOT total supply

### Block Time
- Target: 60 seconds
- Difficulty adjusts every 60 blocks (~1 hour)

### Transaction Fees
- Minimum: 0.00000001 KOT (1 knot)
- Market-determined maximum
- Replace-by-fee: +10% minimum

---

## Building from Source

### Prerequisites

**All Platforms:**
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
- Install Visual Studio 2022 with C++ tools
- Install Rust from rustup.rs

### Build Steps

```bash
# Clone the repository
git clone https://github.com/Ponknot/Knotcoin.git
cd Knotcoin

# Build release binaries
cargo build --release

# Binaries are in target/release/
ls target/release/knotcoind
ls target/release/knotcoin-cli
```

### Run Tests

```bash
cargo test --lib
```

All 45 tests should pass.

---

## RPC API Reference

The node exposes a JSON-RPC API on `http://127.0.0.1:9001`

**Note:** `127.0.0.1` means localhost (your computer only). The RPC port is NOT exposed to the internet for security. Only programs running on your machine can access it.

### Common Commands

**Get blockchain info:**
```bash
curl -X POST http://127.0.0.1:9001 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getmininginfo","params":[],"id":1}'
```

**Get block by height:**
```bash
curl -X POST http://127.0.0.1:9001 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockhash","params":[100],"id":1}'
```

**Get balance:**
```bash
curl -X POST http://127.0.0.1:9001 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getbalance","params":["KOT1..."],"id":1}'
```

### Full API Methods

- `getblockcount` - Current blockchain height
- `getblockhash` - Get block hash by height
- `getblock` - Get full block data
- `getbalance` - Get address balance
- `getmininginfo` - Mining statistics
- `getrawmempool` - Pending transactions
- `sendrawtransaction` - Broadcast transaction
- `generatetoaddress` - Mine blocks
- `getreferralinfo` - Referral statistics
- `getgovernanceinfo` - Governance weight
- `getgovernancetally` - Proposal vote count
- `addnode` - Connect to peer
- `stop` - Shutdown node

---

## Governance

### What Can't Be Changed

These rules are hardcoded. Every node enforces them independently:

- Dilithium3 signatures
- SHA-512 for addresses, SHA3-256 for PoW
- PONC algorithm (2 MB, 512 rounds)
- Emission schedule (all three phases)
- Referral structure (5%, single-hop)
- Block time (60 seconds)
- Minimum block size (50 KB)
- Minimum fee (1 knot)
- No pre-mine

Changing these would create a different system. If you want different rules, fork the code.

### What Can Be Changed

A few operational parameters can be adjusted by vote:

- Block size ceiling (50-500 KB)
- PONC scratchpad size (2-256 MB)
- State channel parameters
- Recommended fees

Changes require >50% of governance weight and activate after 1,000 blocks.

### Who Governs

Anyone who has mined 100+ blocks can vote. Weight is based on blocks mined in the last year, capped at 10% per entity. This prevents any single datacenter from controlling the protocol, even if they have 40% of hashrate.

---

## Security Best Practices

### Wallet Security
1. **Never share your 24-word mnemonic**
2. Write it on paper, store in safe place
3. Never type it on a computer you don't trust
4. Consider splitting it across multiple locations
5. Test recovery before storing large amounts

### Node Security
1. Keep RPC port (9001) local only
2. Use firewall to restrict access
3. Keep software updated
4. Backup `~/.knotcoin/mainnet` regularly
5. Run on dedicated machine if mining seriously

### Network Security
1. Use VPN if concerned about IP exposure
2. Don't reuse addresses (generate new for each transaction)
3. Verify transaction details before signing
4. Be cautious of phishing attempts

---

## Troubleshooting

### Node won't start
```bash
# Check if ports are already in use
lsof -i :9000
lsof -i :9001

# Check logs
tail -f ~/.knotcoin/mainnet/debug.log
```

### Blockchain won't sync
```bash
# Add a peer manually
./knotcoin-cli addnode 192.168.1.100:9000

# Check peer count
./knotcoin-cli getpeerinfo
```

### Mining is slow
- Mining speed depends on your CPU and RAM
- Expected: 1-10 blocks per hour on consumer hardware
- Memory-hard algorithm prevents ASIC dominance
- GPU mining is faster but not required

### Transaction stuck
```bash
# Check mempool
./knotcoin-cli getrawmempool

# Replace with higher fee (RBF)
# Increase fee by at least 10%
```

---

## FAQ

**Q: Is this Bitcoin?**
A: No. Knotcoin is a new blockchain with quantum-resistant signatures and memory-hard mining.

**Q: Can I mine with a GPU?**
A: Yes, but the advantage is only 50-150x (not millions like Bitcoin ASICs). CPUs remain competitive.

**Q: What happens if I lose my 24-word mnemonic?**
A: Your funds are permanently lost. There is no recovery mechanism. Write it down carefully.

**Q: Is there a mobile wallet?**
A: Not yet. Currently desktop only (web interface + CLI).

**Q: When Layer 2 state channels?**
A: Planned for Phase 2. Current focus is stable Layer 1.

**Q: Can I run multiple nodes?**
A: Yes, but they'll compete for the same blocks. Better to run one powerful node.

**Q: How do I upgrade?**
A: Download new binary, stop old node, start new node. Blockchain data is compatible.

**Q: Is there a block explorer?**
A: Yes, the built-in web UI at `share/explorer/index.html` is a full explorer.

**Q: Can I change my referrer?**
A: No. It's set permanently in your first transaction.

**Q: What if my referrer stops mining?**
A: You stop receiving bonuses until they resume (48-hour window).

---

## Technical Specifications

### Cryptography
- **Signatures**: Dilithium3 (NIST FIPS 204)
- **Hashing**: SHA3-256 (NIST FIPS 202) for PoW
- **Addresses**: SHA-512 (first 32 bytes)
- **Key Sizes**: PK=1952 bytes, SK=4032 bytes, Sig=3309 bytes

### Consensus
- **Algorithm**: PONC (Proof of Network Confidence)
- **Scratchpad**: 2 MB (65,536 × 32-byte chunks)
- **Rounds**: 512 per nonce
- **Target**: 60-second blocks
- **Difficulty**: Adjusts every 60 blocks

### Network
- **Protocol**: Custom binary protocol
- **Magic Bytes**: 0x4B4E4F54 ("KNOT")
- **Max Message**: 8 MB
- **Max Peers**: 64 inbound, 8 outbound

### Storage
- **Database**: Sled (embedded key-value store)
- **Block Size**: 50 KB target, 500 KB max
- **Transaction Size**: ~5.4 KB (Dilithium3 signature)

---

## Contributing

Knotcoin is designed to be deployed and left alone. However, bug fixes and security improvements are welcome.

**Before Contributing:**
1. Read the whitepaper (`share/explorer/whitepaper.html`)
2. Understand the eternal rules (cannot be changed)
3. Run all tests (`cargo test`)
4. Follow Rust best practices

**Pull Request Guidelines:**
- Clear description of what and why
- All tests must pass
- No breaking changes to consensus rules
- Security-focused mindset

---

## License

MIT License - See LICENSE file for details

---

## Resources

- **Whitepaper**: `share/explorer/whitepaper.html`
- **GitHub**: https://github.com/Ponknot/Knotcoin
- **Explorer**: `share/explorer/index.html`
- **NIST Dilithium**: https://csrc.nist.gov/pubs/fips/204/final

---

## Philosophy

Three things matter:

1. **Accessible mining**: Memory-hard PoW means your laptop can compete with datacenters. The advantage ratio is 3-8x, not millions like Bitcoin ASICs.

2. **Fair launch**: No pre-mine, no ICO, no founder coins. The creator mines block zero like everyone else. Referral bonuses (5%) reward network growth without creating pyramid schemes.

3. **Long-term survival**: Quantum-resistant signatures, immutable rules, no central authority. Deploy and walk away. The network runs itself.

Quantum resistance is one feature, not the main point. The main point is building something that can't be captured or killed.

---

**Version**: 1.0.0
**Release Date**: February 24, 2026
**Status**: Production Ready

---

*Built to outlast its creator.*
