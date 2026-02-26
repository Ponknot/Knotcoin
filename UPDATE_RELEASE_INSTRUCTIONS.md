# How to Update v1.0.1 Release with Explorer Fix

## What Happened
The v1.0.1 release had a working node but broken web explorer UI. The explorer showed "OFFLINE" because it was trying to connect directly to the RPC server without authentication.

## What Was Fixed
- Changed `app.js` to use `/rpc` proxy endpoint instead of direct connection
- Users must now run `node server.js` instead of `python3 -m http.server`

## Steps to Update GitHub Release

### 1. Delete old release assets
Go to: https://github.com/Knotcoin/knotcoin/releases/tag/v1.0.1

Click "Edit release" and delete these files:
- knotcoin-v1.0.1-linux-x86_64.tar.gz
- knotcoin-v1.0.1-macos-apple-silicon.tar.gz
- knotcoin-v1.0.1-macos-intel.tar.gz
- knotcoin-v1.0.1-windows-x86_64.tar.gz
- RELEASE_CHECKSUMS.txt

### 2. Upload new fixed archives
Upload these files from `dist/` folder:
- knotcoin-v1.0.1-linux-x86_64.tar.gz (FIXED)
- knotcoin-v1.0.1-macos-apple-silicon.tar.gz (FIXED)
- knotcoin-v1.0.1-macos-intel.tar.gz (FIXED)
- knotcoin-v1.0.1-windows-x86_64.tar.gz (FIXED)
- RELEASE_CHECKSUMS_v1.0.1_FIXED.txt (rename to RELEASE_CHECKSUMS.txt)

### 3. Update release notes
Add this at the TOP of the release description:

```
⚠️ EXPLORER FIX APPLIED (2026-02-26)
The initial v1.0.1 release had a non-functional web explorer. This has been fixed.
If you downloaded before Feb 26, 2026 7:30 PM UTC, please re-download.

WHAT CHANGED:
- Fixed web explorer connection issue
- Explorer now uses Node.js server with RPC proxy
- Node binary unchanged (no need to restart if already running)

HOW TO USE EXPLORER:
cd explorer/
node server.js
# Open http://localhost:8080/
```

### 4. Post update on Bitcointalk
Reply to your announcement thread with:

```
UPDATE: v1.0.1 Explorer Fix Applied

The web explorer in the initial v1.0.1 release was not working correctly. This has been fixed.

If you downloaded v1.0.1 before Feb 26, 2026 7:30 PM UTC, please re-download from:
https://github.com/Knotcoin/knotcoin/releases/tag/v1.0.1

What changed:
- Fixed web explorer showing "OFFLINE"
- Explorer now requires Node.js server (instructions in setup guide)
- Node binary is unchanged - if your node is running, no need to restart

The command-line tools (knotcoind, knotcoin-cli) were never affected and work perfectly.

New checksums are in the release.
```

## Alternative: Create v1.0.2 Instead

If you prefer a clean version bump:

```bash
# Bump version to 1.0.2
./bump_version.sh 1.0.2

# Rebuild all platforms
./build_release_all_platforms.sh

# Create new GitHub release v1.0.2
# Mark v1.0.1 as "deprecated - use v1.0.2"
```

## For Users Who Already Downloaded v1.0.1

They have two options:

**Option A: Just update the explorer (quick)**
```bash
cd knotcoin-v1.0.1-*/explorer/
# Download fixed app.js
curl -O https://raw.githubusercontent.com/Knotcoin/knotcoin/main/share/explorer/app.js
# Start Node.js server
node server.js
```

**Option B: Re-download full release**
Download the updated v1.0.1 release from GitHub.

## Summary

- Node binary: ✅ Works perfectly (unchanged)
- CLI tools: ✅ Work perfectly (unchanged)  
- Explorer: ❌ Was broken → ✅ Now fixed

Users only need to update if they want to use the web explorer UI.
