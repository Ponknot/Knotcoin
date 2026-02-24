# Knotcoin Installation Guide

Step-by-step instructions for installing Knotcoin on any platform.

---

## macOS (Apple Silicon / M1/M2/M3)

### Download
1. Go to: https://github.com/Ponknot/Knotcoin/releases
2. Download: `knotcoin-v1.0-macos-arm64.tar.gz`

### Install
```bash
# Extract the archive
tar -xzf knotcoin-v1.0-macos-arm64.tar.gz

# Make executable
chmod +x knotcoind-macos-arm64
chmod +x knotcoin-cli-macos-arm64

# Move to /usr/local/bin (optional, for system-wide access)
sudo mv knotcoind-macos-arm64 /usr/local/bin/knotcoind
sudo mv knotcoin-cli-macos-arm64 /usr/local/bin/knotcoin-cli
```

### Run
```bash
# Start the node
knotcoind

# Or if you didn't move to /usr/local/bin:
./knotcoind-macos-arm64
```

### First Time Setup
macOS may block the app because it's not from the App Store.

**Fix:**
1. Try to run: `./knotcoind-macos-arm64`
2. macOS will show: "Cannot be opened because it is from an unidentified developer"
3. Go to: System Settings → Privacy & Security
4. Scroll down, click "Open Anyway" next to the Knotcoin message
5. Click "Open" in the confirmation dialog
6. Run again: `./knotcoind-macos-arm64`

---

## macOS (Intel)

### Download
1. Go to: https://github.com/Ponknot/Knotcoin/releases
2. Download: `knotcoin-v1.0-macos-x64.tar.gz`

### Install
```bash
# Extract
tar -xzf knotcoin-v1.0-macos-x64.tar.gz

# Make executable
chmod +x knotcoind-macos-x64
chmod +x knotcoin-cli-macos-x64

# Move to system path (optional)
sudo mv knotcoind-macos-x64 /usr/local/bin/knotcoind
sudo mv knotcoin-cli-macos-x64 /usr/local/bin/knotcoin-cli
```

### Run
```bash
knotcoind
```

Follow the same "First Time Setup" steps as Apple Silicon if blocked.

---

## Linux (Ubuntu/Debian)

### Download
```bash
# Download latest release
wget https://github.com/Ponknot/Knotcoin/releases/download/v1.0/knotcoin-v1.0-linux-x64.tar.gz

# Extract
tar -xzf knotcoin-v1.0-linux-x64.tar.gz

# Make executable
chmod +x knotcoind-linux-x64
chmod +x knotcoin-cli-linux-x64
```

### Install System-Wide (Optional)
```bash
sudo mv knotcoind-linux-x64 /usr/local/bin/knotcoind
sudo mv knotcoin-cli-linux-x64 /usr/local/bin/knotcoin-cli
```

### Run
```bash
# Start node
knotcoind

# Or from current directory:
./knotcoind-linux-x64
```

### Run as Service (Optional)

Create a systemd service file:

```bash
sudo nano /etc/systemd/system/knotcoind.service
```

Add this content:
```ini
[Unit]
Description=Knotcoin Node
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
ExecStart=/usr/local/bin/knotcoind
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable knotcoind
sudo systemctl start knotcoind

# Check status
sudo systemctl status knotcoind

# View logs
sudo journalctl -u knotcoind -f
```

---

## Windows

### Download
1. Go to: https://github.com/Ponknot/Knotcoin/releases
2. Download: `knotcoin-v1.0-windows-x64.zip`

### Install
1. Right-click the ZIP file
2. Select "Extract All..."
3. Choose a location (e.g., `C:\Knotcoin`)
4. Click "Extract"

### Run
1. Open the extracted folder
2. Double-click `knotcoind-windows-x64.exe`

**Or use Command Prompt:**
```cmd
cd C:\Knotcoin
knotcoind-windows-x64.exe
```

### Windows Defender Warning
Windows may show a SmartScreen warning.

**Fix:**
1. Click "More info"
2. Click "Run anyway"

This happens because the binary isn't signed with a Microsoft certificate (costs $300/year).

### Add to PATH (Optional)

To run `knotcoind` from anywhere:

1. Press `Win + X`, select "System"
2. Click "Advanced system settings"
3. Click "Environment Variables"
4. Under "System variables", find "Path"
5. Click "Edit"
6. Click "New"
7. Add: `C:\Knotcoin` (or wherever you extracted)
8. Click "OK" on all dialogs
9. Restart Command Prompt

Now you can run:
```cmd
knotcoind
```

---

## Building from Source (All Platforms)

If you prefer to build from source or need a different platform:

### Prerequisites

**Install Rust:**
```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows: Download from https://rustup.rs
```

**Install C++ Compiler:**

macOS:
```bash
xcode-select --install
```

Linux (Ubuntu/Debian):
```bash
sudo apt update
sudo apt install build-essential cmake
```

Windows:
- Install Visual Studio 2022 Community Edition
- Select "Desktop development with C++"

### Build Steps

```bash
# Clone repository
git clone https://github.com/Ponknot/Knotcoin.git
cd Knotcoin

# Build release binaries
cargo build --release

# Binaries are in:
# target/release/knotcoind
# target/release/knotcoin-cli
```

### Run Tests
```bash
cargo test --lib
```

All 45 tests should pass.

---

## Verifying Your Installation

After installation, verify everything works:

### 1. Check Version
```bash
knotcoind --version
# Should output: knotcoind 1.0.0
```

### 2. Start Node
```bash
knotcoind
```

You should see:
```
Knotcoin Node v1.0.0
Initializing database...
Starting P2P server on 0.0.0.0:9000
Starting RPC server on 127.0.0.1:9001
Node running. Press Ctrl+C to stop.
```

### 3. Test RPC (in another terminal)
```bash
curl -X POST http://127.0.0.1:9001 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}'
```

Should return:
```json
{"jsonrpc":"2.0","result":0,"id":1}
```

### 4. Open Web Explorer

**Option A: Direct File**
```
file:///path/to/Knotcoin/share/explorer/index.html
```

**Option B: Python Server**
```bash
cd share/explorer
python3 -m http.server 8080
```

Then visit: `http://localhost:8080`

---

## Troubleshooting

### "Command not found"
- Make sure you ran `chmod +x` on the binary
- Check if you're in the correct directory
- Try using `./knotcoind` instead of `knotcoind`

### "Permission denied"
```bash
chmod +x knotcoind-*
```

### "Port already in use"
Another program is using port 9000 or 9001.

**Find what's using the port:**

macOS/Linux:
```bash
lsof -i :9000
lsof -i :9001
```

Windows:
```cmd
netstat -ano | findstr :9000
netstat -ano | findstr :9001
```

**Solution:** Stop the other program or change Knotcoin's ports in the config.

### "Cannot connect to RPC"
- Make sure `knotcoind` is running
- Check if port 9001 is open: `curl http://127.0.0.1:9001`
- Check firewall settings

### macOS "Damaged and can't be opened"
```bash
# Remove quarantine attribute
xattr -d com.apple.quarantine knotcoind-macos-arm64
```

### Linux "error while loading shared libraries"
```bash
# Install missing libraries
sudo apt install libssl-dev
```

---

## Uninstalling

### macOS/Linux
```bash
# Stop the node
pkill knotcoind

# Remove binaries
sudo rm /usr/local/bin/knotcoind
sudo rm /usr/local/bin/knotcoin-cli

# Remove data (WARNING: This deletes your blockchain)
rm -rf ~/.knotcoin
```

### Windows
1. Stop `knotcoind.exe` (close the window or Ctrl+C)
2. Delete the `C:\Knotcoin` folder
3. Delete `C:\Users\YourName\.knotcoin` (blockchain data)

---

## Next Steps

After installation:

1. **Create a wallet**: See README.md "Creating Your First Wallet"
2. **Mine your first block**: See README.md "Mining Your First Block"
3. **Join the network**: Connect to other nodes
4. **Read the whitepaper**: `share/explorer/whitepaper.html`

---

## Getting Help

- **GitHub Issues**: https://github.com/Ponknot/Knotcoin/issues
- **Documentation**: README.md
- **Whitepaper**: share/explorer/whitepaper.html

---

**Remember:** Write down your 24-word mnemonic and store it safely. It's the only way to recover your wallet.
