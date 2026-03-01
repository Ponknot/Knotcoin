#!/usr/bin/env bash
# Knotcoin seed-node guard: only start if not already running on port 9000.
# This prevents conflicts between the LaunchAgent and Electron.

BINARY="/Users/illumoking/Desktop/KNOTCOIN/target/release/knotcoind"
DATA_DIR="$HOME/.knotcoin/mainnet"

# If something is already listening on port 9000, exit cleanly (code 0)
# so launchd KeepAlive does NOT restart us.
if lsof -i TCP:9000 -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "[guard] port 9000 already in use, skipping start" >&2
    exit 0
fi

mkdir -p "$DATA_DIR"
rm -f "$DATA_DIR/chaindata/LOCK"

exec "$BINARY" --rpc-port=9001 --p2p-port=9000
