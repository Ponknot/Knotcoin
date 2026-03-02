#!/bin/bash
# Simple launcher for Knotcoin - starts knotcoind and opens UI in browser

set -e

echo "Starting Knotcoin..."

# Detect platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    BINARY="./knotcoind"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    BINARY="./knotcoind"
else
    BINARY="./knotcoind.exe"
fi

# Start knotcoind in background
$BINARY --rpc-port=9001 --p2p-port=9000 &
KNOTCOIN_PID=$!

echo "Knotcoin node started (PID: $KNOTCOIN_PID)"
echo "Opening UI in browser..."

# Wait for node to start
sleep 3

# Open browser
if [[ "$OSTYPE" == "darwin"* ]]; then
    open "file://$(pwd)/share/explorer/index.html"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    xdg-open "file://$(pwd)/share/explorer/index.html"
else
    start "file://$(pwd)/share/explorer/index.html"
fi

echo ""
echo "Knotcoin is running!"
echo "To stop: kill $KNOTCOIN_PID"
echo ""

# Keep script running
wait $KNOTCOIN_PID
