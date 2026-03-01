#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN_D="$ROOT_DIR/target/release/knotcoind"
BIN_C="$ROOT_DIR/target/release/knotcoin-cli"

RPC_A=19011
P2P_A=19010
DIR_A=/tmp/knot-A

RPC_B=19021
P2P_B=19020
DIR_B=/tmp/knot-B

RPC_C=19031
P2P_C=19030
DIR_C=/tmp/knot-C

kill_port_listeners() {
  local port="$1"
  local pids
  pids="$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    kill $pids 2>/dev/null || true
    sleep 0.2
  fi
}

cleanup() {
  set +e
  [[ -n "${PID_A:-}" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "${PID_B:-}" ]] && kill "$PID_B" 2>/dev/null || true
  [[ -n "${PID_C:-}" ]] && kill "$PID_C" 2>/dev/null || true
}
trap cleanup EXIT

if [[ ! -x "$BIN_D" || ! -x "$BIN_C" ]]; then
  echo "Missing binaries. Build first:" >&2
  echo "  cargo build --release --bin knotcoind --bin knotcoin-cli" >&2
  exit 1
fi

rm -rf "$DIR_A" "$DIR_B" "$DIR_C"
mkdir -p "$DIR_A" "$DIR_B" "$DIR_C"

kill_port_listeners "$RPC_A"
kill_port_listeners "$P2P_A"
kill_port_listeners "$RPC_B"
kill_port_listeners "$P2P_B"
kill_port_listeners "$RPC_C"
kill_port_listeners "$P2P_C"

KNOTCOIN_DATA_DIR="$DIR_A" KNOTCOIN_RPC_PORT="$RPC_A" KNOTCOIN_P2P_PORT="$P2P_A" \
KNOTCOIN_DEV_ALLOW_LOCAL=1 \
  "$BIN_D" --rpc-port="$RPC_A" --p2p-port="$P2P_A" --data-dir="$DIR_A" >/tmp/knot-A.log 2>/tmp/knot-A.err &
PID_A=$!

KNOTCOIN_DATA_DIR="$DIR_B" KNOTCOIN_RPC_PORT="$RPC_B" KNOTCOIN_P2P_PORT="$P2P_B" \
KNOTCOIN_DEV_ALLOW_LOCAL=1 \
  "$BIN_D" --rpc-port="$RPC_B" --p2p-port="$P2P_B" --data-dir="$DIR_B" >/tmp/knot-B.log 2>/tmp/knot-B.err &
PID_B=$!

KNOTCOIN_DATA_DIR="$DIR_C" KNOTCOIN_RPC_PORT="$RPC_C" KNOTCOIN_P2P_PORT="$P2P_C" \
KNOTCOIN_DEV_ALLOW_LOCAL=1 \
  "$BIN_D" --rpc-port="$RPC_C" --p2p-port="$P2P_C" --data-dir="$DIR_C" >/tmp/knot-C.log 2>/tmp/knot-C.err &
PID_C=$!

wait_cookie() {
  local dir="$1"
  local tries=100
  while [[ $tries -gt 0 ]]; do
    [[ -f "$dir/.cookie" ]] && [[ $(wc -c < "$dir/.cookie" | tr -d ' ') -gt 10 ]] && return 0
    sleep 0.2
    tries=$((tries-1))
  done
  return 1
}

wait_cookie "$DIR_A" || { echo "cookie not created for A" >&2; exit 1; }
wait_cookie "$DIR_B" || { echo "cookie not created for B" >&2; exit 1; }
wait_cookie "$DIR_C" || { echo "cookie not created for C" >&2; exit 1; }

KNOTCOIN_DATA_DIR="$DIR_B" KNOTCOIN_RPC_PORT="$RPC_B" "$BIN_C" addnode "127.0.0.1:$P2P_A" >/dev/null
KNOTCOIN_DATA_DIR="$DIR_C" KNOTCOIN_RPC_PORT="$RPC_C" "$BIN_C" addnode "127.0.0.1:$P2P_B" >/dev/null

sleep 25

echo "=== peers.json ==="
for d in "$DIR_A" "$DIR_B" "$DIR_C"; do
  echo "--- $d ---"
  if [[ -f "$d/peers.json" ]]; then
    cat "$d/peers.json"
  else
    echo "(missing peers.json)"
  fi
done

echo "=== tail logs (p2p) ==="
for n in A B C; do
  echo "--- node $n ---"
  tail -n 40 "/tmp/knot-$n.log" 2>/dev/null || true
  tail -n 40 "/tmp/knot-$n.err" 2>/dev/null || true
done
