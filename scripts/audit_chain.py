#!/usr/bin/env python3
"""Audit blockchain data: scan all blocks and compare with account state."""
import json
import urllib.request

RPC_URL = "http://127.0.0.1:9001/rpc"
TOKEN = open("/Users/illumoking/.knotcoin/mainnet/.cookie").read().strip()

def rpc(method, params=[]):
    data = json.dumps({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).encode()
    req = urllib.request.Request(RPC_URL, data=data, headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {TOKEN}"
    })
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read()).get("result")

# Get chain height
height = rpc("getblockcount")
print(f"Chain height: {height}")

# Scan all blocks
miner_blocks = {}
for h in range(1, height + 1):
    block_hash = rpc("getblockhash", [h])
    if block_hash:
        block = rpc("getblock", [block_hash])
        if block:
            miner = block.get("miner", "unknown")
            miner_blocks[miner] = miner_blocks.get(miner, 0) + 1

print(f"\n=== ACTUAL BLOCKS PER MINER (from blockchain scan) ===")
total_scanned = 0
for miner, count in sorted(miner_blocks.items(), key=lambda x: -x[1]):
    print(f"  {miner[:24]}... : {count:3d} blocks")
    total_scanned += count
print(f"Total blocks scanned: {total_scanned}")

# Get miners from RPC
print(f"\n=== FROM get_all_miners RPC ===")
miners_rpc = rpc("get_all_miners")
if miners_rpc:
    miners = miners_rpc.get("miners", [])
    print(f"Total miners from RPC: {len(miners)}")
    total_rpc = 0
    for m in sorted(miners, key=lambda x: x.get("total_blocks_mined", 0), reverse=True):
        addr = m["address"]
        blocks = m.get("total_blocks_mined", 0)
        last_h = m.get("last_mined_height", 0)
        actual = miner_blocks.get(addr, 0)
        match = "✓" if blocks == actual else f"✗ (actual={actual})"
        print(f"  {addr[:24]}... : {blocks:3d} blocks, last_h={last_h:3d} {match}")
        total_rpc += blocks
    print(f"Total blocks from RPC: {total_rpc}")

print(f"\n=== SUMMARY ===")
print(f"Chain height: {height}")
print(f"Blocks scanned: {total_scanned}")
print(f"Unique miners in chain: {len(miner_blocks)}")
print(f"Miners in RPC: {len(miners) if miners_rpc else 0}")
