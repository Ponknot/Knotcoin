#!/usr/bin/env bash
set -euo pipefail

# Knotcoin Seed Node — macOS LaunchAgent installer
# This makes knotcoind start automatically on login and restart if it crashes.

BINARY="${1:-/Users/illumoking/Desktop/KNOTCOIN/target/release/knotcoind}"
DATA_DIR="$HOME/.knotcoin/mainnet"
PLIST_NAME="com.knotcoin.seed-node"
PLIST_PATH="$HOME/Library/LaunchAgents/${PLIST_NAME}.plist"
LOG_DIR="$HOME/.knotcoin/logs"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: knotcoind binary not found at $BINARY"
  echo "Usage: $0 /path/to/knotcoind"
  exit 1
fi

mkdir -p "$LOG_DIR" "$DATA_DIR"

# Unload existing if present
launchctl bootout gui/$(id -u) "$PLIST_PATH" 2>/dev/null || true

cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${PLIST_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${BINARY}</string>
        <string>--rpc-port=9001</string>
        <string>--p2p-port=9000</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>KNOTCOIN_DATA_DIR</key>
        <string>${DATA_DIR}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>${LOG_DIR}/knotcoind-stdout.log</string>
    <key>StandardErrorPath</key>
    <string>${LOG_DIR}/knotcoind-stderr.log</string>
    <key>ThrottleInterval</key>
    <integer>5</integer>
</dict>
</plist>
EOF

launchctl bootstrap gui/$(id -u) "$PLIST_PATH"

echo "Seed node installed and started."
echo "  Binary:  $BINARY"
echo "  Data:    $DATA_DIR"
echo "  Logs:    $LOG_DIR/"
echo "  Plist:   $PLIST_PATH"
echo ""
echo "It will auto-start on login and restart if it crashes."
echo "To stop:   launchctl bootout gui/\$(id -u) $PLIST_PATH"
echo "To check:  ps aux | grep knotcoind"
