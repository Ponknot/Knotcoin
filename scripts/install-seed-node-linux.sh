#!/usr/bin/env bash
set -euo pipefail

# Knotcoin Seed Node — Linux systemd service installer
# Usage: sudo ./install-seed-node-linux.sh [/path/to/knotcoind]

BINARY="${1:-/usr/local/bin/knotcoind}"
DATA_DIR="$HOME/.knotcoin/mainnet"
SERVICE_NAME="knotcoin-seed"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
USER_NAME="$(whoami)"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: knotcoind binary not found at $BINARY"
  echo "Usage: $0 /path/to/knotcoind"
  exit 1
fi

mkdir -p "$DATA_DIR"

cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=Knotcoin Node (Seed)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${USER_NAME}
ExecStart=${BINARY} --rpc-port=9001 --p2p-port=9000
Restart=on-failure
RestartSec=5
Environment=KNOTCOIN_DATA_DIR=${DATA_DIR}
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME"
systemctl restart "$SERVICE_NAME"

echo "Seed node installed as systemd service: $SERVICE_NAME"
echo "  Status:  systemctl status $SERVICE_NAME"
echo "  Logs:    journalctl -u $SERVICE_NAME -f"
echo "  Stop:    systemctl stop $SERVICE_NAME"
