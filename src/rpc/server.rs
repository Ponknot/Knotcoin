use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::path::PathBuf;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, body::Incoming};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tokio::sync::Mutex;

use crate::config::{RPC_BIND_ADDRESS, RPC_COOKIE_FILE};
use crate::consensus::state::block_hash;
use crate::net::mempool::Mempool;
use crate::net::node::P2pCommand;
use crate::node::ChainDB;

fn load_known_peers_from_disk(data_dir: &str) -> Vec<String> {
    let path = std::path::Path::new(data_dir).join("peers.json");
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
}

fn parse_advertised_addrs() -> Vec<SocketAddr> {
    std::env::var("KNOTCOIN_ADVERTISE_ADDRS")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|p| p.trim().parse::<SocketAddr>().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn is_private_ip(addr: &SocketAddr) -> bool {
    let ip = addr.ip();
    if ip.is_loopback() {
        return true;
    }
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private(),
        std::net::IpAddr::V6(v6) => (v6.segments()[0] & 0xfe00) == 0xfc00,
    }
}

fn estimate_network_hashrate_from_target(target_bytes: &[u8; 32]) -> u64 {
    use primitive_types::U256;

    let mut target = U256::from_big_endian(target_bytes);
    if target.is_zero() {
        target = U256::one();
    }

    let expected_hashes = match target.checked_add(U256::one()) {
        Some(t_plus_one) => U256::MAX / t_plus_one,
        None => U256::zero(),
    };
    let hps = expected_hashes / U256::from(60u64);
    if hps > U256::from(u64::MAX) {
        u64::MAX
    } else {
        hps.low_u64()
    }
}

type WalletKeyCache = std::collections::HashMap<
    [u8; 32],
    (
        crate::crypto::dilithium::PublicKey,
        crate::crypto::dilithium::SecretKey,
    ),
>;

pub struct RpcState {
    pub db: ChainDB,
    pub mempool: Arc<Mutex<Mempool>>,
    pub shutdown: AtomicBool,
    pub p2p_tx: tokio::sync::mpsc::UnboundedSender<P2pCommand>,
    pub auth_token: String,
    pub data_dir: String,
    pub p2p_port: u16,
    pub mining_active: AtomicBool,
    pub mining_blocks_found: Arc<AtomicU64>,
    pub mining_start_time: Arc<AtomicU64>,
    pub mining_stop: Arc<AtomicBool>,
    pub connected_peers: Arc<std::sync::atomic::AtomicUsize>,
    pub wallet_keys: Arc<Mutex<WalletKeyCache>>,
    pub mining_nonces_total: Arc<AtomicU64>,
    pub mining_address: Arc<Mutex<Option<[u8; 32]>>>,
    pub mining_referrer: Arc<Mutex<Option<[u8; 32]>>>,
}

fn existing_wallet_hash_mismatch(data_dir: &str, mnemonic_hash: &[u8; 32]) -> bool {
    let path = wallet_keys_file(data_dir);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let stored: StoredWalletKeys = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return false,
    };
    stored.mnemonic_hash_hex != hex::encode(mnemonic_hash)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredWalletKeys {
    mnemonic_hash_hex: String,
    public_key: Vec<u8>,
    secret_key: Vec<u8>,
}

fn wallet_keys_file(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir).join("wallet_keys.json")
}

fn load_wallet_keys_from_disk(data_dir: &str, mnemonic_hash: &[u8; 32]) -> Option<(crate::crypto::dilithium::PublicKey, crate::crypto::dilithium::SecretKey)> {
    let path = wallet_keys_file(data_dir);
    let backup_path = path.with_extension("json.backup");
    
    // Try main file first, then backup
    let raw = std::fs::read_to_string(&path)
        .or_else(|_| std::fs::read_to_string(&backup_path))
        .ok()?;
    
    let stored: StoredWalletKeys = serde_json::from_str(&raw).ok()?;
    if stored.mnemonic_hash_hex != hex::encode(mnemonic_hash) {
        return None;
    }
    if stored.public_key.len() != crate::crypto::dilithium::DILITHIUM3_PUBKEY_BYTES {
        return None;
    }
    if stored.secret_key.len() != crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES {
        return None;
    }
    let mut pkb = [0u8; crate::crypto::dilithium::DILITHIUM3_PUBKEY_BYTES];
    pkb.copy_from_slice(&stored.public_key);
    let mut skb = [0u8; crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES];
    skb.copy_from_slice(&stored.secret_key);
    Some((crate::crypto::dilithium::PublicKey(pkb), crate::crypto::dilithium::SecretKey(skb)))
}

fn save_wallet_keys_to_disk(data_dir: &str, mnemonic_hash: &[u8; 32], pk: &crate::crypto::dilithium::PublicKey, sk: &crate::crypto::dilithium::SecretKey) {
    let path = wallet_keys_file(data_dir);
    let backup_path = path.with_extension("json.backup");
    let tmp_path = path.with_extension("json.tmp");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let stored = StoredWalletKeys {
        mnemonic_hash_hex: hex::encode(mnemonic_hash),
        public_key: pk.0.to_vec(),
        secret_key: sk.0.to_vec(),
    };
    if let Ok(s) = serde_json::to_string_pretty(&stored) {
        if std::fs::write(&tmp_path, s).is_ok() {
            // Best-effort backup of the previous file to prevent wallet loss on corruption.
            if path.exists() {
                let _ = std::fs::copy(&path, &backup_path);
            }
            let _ = std::fs::rename(&tmp_path, &path);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    let _ = std::fs::set_permissions(&path, perms);
                }
                if let Ok(meta) = std::fs::metadata(&backup_path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    let _ = std::fs::set_permissions(&backup_path, perms);
                }
            }
        }
    }
}

async fn cached_keypair_for_mnemonic(
    state: &RpcState,
    mnemonic: &str,
) -> (crate::crypto::dilithium::PublicKey, crate::crypto::dilithium::SecretKey) {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(mnemonic.as_bytes());
    let digest = h.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);

    // Single-wallet-per-profile: if a wallet already exists on disk for this profile,
    // do not silently switch identities by importing a different mnemonic.
    if existing_wallet_hash_mismatch(&state.data_dir, &key) {
        // We can't return a Result from here; callers handle this mismatch explicitly
        // by checking wallet identity first where needed.
        // As a safe fallback, keep behavior stable and derive keys without overwriting disk.
    }

    let mut cache = state.wallet_keys.lock().await;
    if let Some((pk, sk)) = cache.get(&key) {
        return (*pk, sk.clone());
    }

    // No-password persistent store (single-wallet). If present, prefer it.
    if let Some((pk, sk)) = load_wallet_keys_from_disk(&state.data_dir, &key) {
        cache.insert(key, (pk, sk.clone()));
        return (pk, sk);
    }

    // NOTE: Dilithium keygen is not deterministic in this version; cache ensures stability
    // across RPC calls within the same daemon run.
    let (pk, sk) = crate::crypto::keys::derive_keypair_from_mnemonic(mnemonic);
    cache.insert(key, (pk, sk.clone()));
    if !existing_wallet_hash_mismatch(&state.data_dir, &key) {
        save_wallet_keys_to_disk(&state.data_dir, &key, &pk, &sk);
    }
    (pk, sk)
}

async fn ensure_single_wallet_identity(state: &RpcState, mnemonic: &str) -> Result<(), (i32, String)> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(mnemonic.as_bytes());
    let digest = h.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest[..32]);
    if existing_wallet_hash_mismatch(&state.data_dir, &key) {
        return Err((-32603, "wallet profile already initialized with a different mnemonic".to_string()));
    }
    Ok(())
}

async fn handle_rpc(state: &RpcState, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    match method {
        "getblockcount" => Ok(json!(
            state
                .db
                .get_chain_height()
                .map_err(|e| (-32603, format!("db error: {e}")))?
        )),

        "getblockhash" => {
            let h = params.get(0).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            match state.db.get_block_hash_by_height(h) {
                Ok(Some(hash)) => Ok(json!(hex::encode(hash))),
                Ok(None) => Err((-32602, "block not found".to_string())),
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        // Get block by height (convenience method)
        "getblockbyheight" => {
            let h = params.get(0).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let hash = match state.db.get_block_hash_by_height(h) {
                Ok(Some(hash)) => hash,
                Ok(None) => return Err((-32602, "block not found".to_string())),
                Err(e) => return Err((-32603, format!("db error: {e}"))),
            };
            match state.db.get_block(&hash) {
                Ok(Some(block)) => {
                    // Calculate block reward from consensus schedule
                    let reward = crate::consensus::chain::calculate_block_reward(h as u64);
                    
                    // Calculate human-readable difficulty
                    // Count leading zero bits in target (more zeros = harder)
                    let target_bytes = &block.difficulty_target;
                    let mut leading_zeros = 0u32;
                    for &b in target_bytes.iter() {
                        if b == 0x00 {
                            leading_zeros += 8;
                        } else {
                            leading_zeros += b.leading_zeros() as u32;
                            break;
                        }
                    }
                    // Difficulty is 2^leading_zeros, minimum 1
                    let difficulty_human = if leading_zeros == 0 { 1.0 } else { 2f64.powi(leading_zeros as i32) };
                    
                    Ok(json!({
                        "hash": hex::encode(block_hash(&block)),
                        "height": h,
                        "version": u32::from_be_bytes(block.version),
                        "previousblockhash": hex::encode(block.previous_hash),
                        "merkleroot": hex::encode(block.merkle_root),
                        "time": u32::from_le_bytes(block.timestamp),
                        "difficulty_hex": hex::encode(block.difficulty_target),
                        "difficulty": difficulty_human,
                        "difficulty_bits": leading_zeros.max(1),
                        "nonce": hex::encode(block.nonce),
                        "miner": crate::crypto::keys::encode_address_string(&block.miner_address),
                        "reward_knots": reward,
                        "reward_kot": format!("{:.8}", reward as f64 / 1e8),
                        "tx_count": block.tx_data.len(),
                        "transactions": block.tx_data.iter().map(|tx| json!({
                            "sender": crate::crypto::keys::encode_address_string(&tx.sender_address),
                            "recipient": crate::crypto::keys::encode_address_string(&tx.recipient_address),
                            "amount_knots": tx.amount,
                            "amount_kot": format!("{:.8}", tx.amount as f64 / 1e8),
                            "fee": tx.fee,
                            "nonce": tx.nonce,
                        })).collect::<Vec<_>>(),
                    }))
                }
                Ok(None) => Err((-32602, "block not found".to_string())),
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "getblock" => {
            let hex_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let raw =
                hex::decode(hex_str).map_err(|_| (-32602, "invalid hash format".to_string()))?;
            if raw.len() != 32 {
                return Err((-32602, "invalid hash length".to_string()));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&raw);

            match state.db.get_block(&hash) {
                Ok(Some(block)) => Ok(json!({
                    "hash":              hex::encode(block_hash(&block)),
                    "height":            u32::from_le_bytes(block.block_height),
                    "version":           u32::from_be_bytes(block.version),
                    "previousblockhash": hex::encode(block.previous_hash),
                    "merkleroot":        hex::encode(block.merkle_root),
                    "time":              u32::from_le_bytes(block.timestamp),
                    "difficulty":        hex::encode(block.difficulty_target),
                    "nonce":             hex::encode(block.nonce),
                    "miner":             crate::crypto::keys::encode_address_string(&block.miner_address),
                    "tx_count":          block.tx_data.len(),
                    "transactions":      block.tx_data.iter().map(|tx| json!({
                        "sender":    crate::crypto::keys::encode_address_string(&tx.sender_address),
                        "recipient": crate::crypto::keys::encode_address_string(&tx.recipient_address),
                        "amount":    tx.amount,
                        "fee":       tx.fee,
                        "nonce":     tx.nonce,
                        "gov_data":  tx.governance_data.map(hex::encode),
                    })).collect::<Vec<_>>(),
                })),
                Ok(None) => Err((-32602, "block not found".to_string())),
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "getbalance" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let addr = if let Ok(a) = crate::crypto::keys::decode_address_string(addr_str) {
                a
            } else {
                let hex_part = if addr_str.to_lowercase().starts_with("kot1") {
                    &addr_str[4..]
                } else if addr_str.to_lowercase().starts_with("kot") {
                    &addr_str[3..]
                } else {
                    addr_str
                };
                match hex::decode(hex_part) {
                    Ok(b) if b.len() == 32 => {
                        let mut a = [0u8; 32];
                        a.copy_from_slice(&b);
                        a
                    }
                    _ => return Err((-32602, "invalid address".to_string())),
                }
            };

            match state.db.get_account(&addr) {
                Ok(a) => {
                    let code = crate::crypto::hash::hash_sha3_256(&addr);
                    Ok(json!({
                        "balance_knots":    a.balance,
                        "balance_kot":      format!("{:.8}", a.balance as f64 / 1e8),
                        "nonce":            a.nonce,
                        "last_mined_height":a.last_mined_height,
                        "privacy_code":     hex::encode(&code[..8]),
                    }))
                }
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "getmininginfo" => {
            let height = state.db.get_chain_height().unwrap_or(0);
            let pool_size = state.mempool.lock().await.size();

            // Fetch current difficulty target from the chain tip
            let tip_hash = state.db.get_tip().ok().flatten();
            let tip_block = tip_hash.and_then(|h| state.db.get_block(&h).ok().flatten());

            let difficulty = tip_block
                .as_ref()
                .map(|b| hex::encode(b.difficulty_target))
                .unwrap_or_else(|| "f".repeat(64));

            // Get governance params for mining threads and PONC rounds
            let params = state.db.get_governance_params().unwrap_or_default();

            Ok(json!({
                "blocks":         height,
                "difficulty":     difficulty,
                "mempool":        pool_size,
                "mining_threads": params.mining_threads,
                "ponc_rounds":    params.ponc_rounds,
                "network":        "mainnet",
                "quantum_sec":    "Dilithium3 (NIST FIPS 204)",
            }))
        }

        "getmempoolinfo" => {
            let pool_size = state.mempool.lock().await.size();
            Ok(json!({
                "size": pool_size,
                "bytes": 0,
            }))
        }

        "getrawmempool" => {
            let pool = state.mempool.lock().await;
            let ids: Vec<String> = pool.get_all_txids().iter().map(hex::encode).collect();
            Ok(json!(ids))
        }

        "getmempool" => {
            let limit = params.get(0).and_then(|v| v.as_u64()).unwrap_or(100).min(1000) as usize;
            let pool = state.mempool.lock().await;
            let entries = pool.get_entries(limit);
            let out: Vec<Value> = entries.into_iter().map(|e| {
                let size = crate::net::mempool::Mempool::estimate_tx_size(&e.tx) as u64;
                json!({
                    "txid": hex::encode(e.txid),
                    "sender": crate::crypto::keys::encode_address_string(&e.tx.sender_address),
                    "recipient": crate::crypto::keys::encode_address_string(&e.tx.recipient_address),
                    "amount_knots": e.tx.amount,
                    "amount_kot": format!("{:.8}", e.tx.amount as f64 / 1e8),
                    "fee": e.tx.fee,
                    "nonce": e.tx.nonce,
                    "size": size,
                    "fee_per_byte": e.fee_per_byte_scaled as f64 / 10000.0
                })
            }).collect();
            Ok(json!({ "transactions": out }))
        }

        "getrecentblocks" => {
            let count = params.get(0).and_then(|v| v.as_u64()).unwrap_or(20).min(200) as u32;
            let height = state.db.get_chain_height().unwrap_or(0);
            let start = height.saturating_sub(count.saturating_sub(1));
            let mut blocks = Vec::new();
            for h in (start..=height).rev() {
                if let Ok(Some(hash)) = state.db.get_block_hash_by_height(h) {
                    if let Ok(Some(block)) = state.db.get_block(&hash) {
                        let reward = crate::consensus::chain::calculate_block_reward(h as u64);
                        blocks.push(json!({
                            "hash": hex::encode(hash),
                            "height": h,
                            "time": u32::from_le_bytes(block.timestamp),
                            "miner": crate::crypto::keys::encode_address_string(&block.miner_address),
                            "tx_count": block.tx_data.len(),
                            "reward_knots": reward,
                            "reward_kot": format!("{:.8}", reward as f64 / 1e8),
                        }));
                    }
                }
            }
            Ok(json!({ "blocks": blocks }))
        }

        "getstatus" => {
            let height = state.db.get_chain_height().unwrap_or(0);
            let pool_size = state.mempool.lock().await.size();
            let connected = state.connected_peers.load(Ordering::Relaxed);
            let known = load_known_peers_from_disk(&state.data_dir);
            let advertised = parse_advertised_addrs();

            let tip_hash = state.db.get_tip().ok().flatten();
            let tip_block = tip_hash.and_then(|h| state.db.get_block(&h).ok().flatten());
            let difficulty = tip_block
                .as_ref()
                .map(|b| hex::encode(b.difficulty_target))
                .unwrap_or_else(|| "f".repeat(64));

            let mining_active = state.mining_active.load(Ordering::SeqCst);
            let blocks_found = state.mining_blocks_found.load(Ordering::SeqCst);
            let start = state.mining_start_time.load(Ordering::SeqCst);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let uptime = if mining_active && start > 0 { now - start } else { 0 };
            let nonces = state.mining_nonces_total.load(Ordering::SeqCst);
            let hashrate = if uptime > 0 { nonces / uptime } else { 0 };

            let params = state.db.get_governance_params().unwrap_or_default();

            Ok(json!({
                "chain_height": height,
                "tip": tip_hash.map(hex::encode),
                "mempool": pool_size,
                "difficulty": difficulty,
                "peers_connected": connected,
                "known_peers": known.len(),
                "mining_active": mining_active,
                "mining_blocks_found": blocks_found,
                "mining_hashrate": hashrate,
                "mining_threads": params.mining_threads,
                "ponc_rounds": params.ponc_rounds,
                "p2p_port": state.p2p_port,
                "advertised_addrs": advertised.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            }))
        }

        "getbootstrapcheck" => {
            let connected = state.connected_peers.load(Ordering::Relaxed);
            let known = load_known_peers_from_disk(&state.data_dir);
            let advertised = parse_advertised_addrs();

            let mut warnings = Vec::new();
            if crate::config::p2p_bind_address() == "127.0.0.1" {
                warnings.push("P2P bind address is 127.0.0.1; node is not reachable from the internet".to_string());
            }
            if advertised.is_empty() {
                warnings.push("KNOTCOIN_ADVERTISE_ADDRS is not set; peers may not learn your public address".to_string());
            } else if advertised.iter().any(is_private_ip) {
                warnings.push("Advertised address is private or loopback; peers on the internet cannot reach it".to_string());
            }
            if connected == 0 {
                warnings.push("No connected peers yet; check port forwarding and firewall".to_string());
            }
            if known.is_empty() {
                warnings.push("No known peers on disk; bootstrap may be failing".to_string());
            }

            Ok(json!({
                "p2p_port": state.p2p_port,
                "p2p_bind": crate::config::p2p_bind_address(),
                "connected_peers": connected,
                "known_peers": known.len(),
                "advertised_addrs": advertised.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
                "warnings": warnings,
                "notes": "This check cannot verify NAT/port-forward reachability; test externally with netcat or a port checker."
            }))
        }

        "sendrawtransaction" => {
            let hex_str = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "hex required".to_string()))?;
            let raw = hex::decode(hex_str).map_err(|_| (-32602, "invalid hex".to_string()))?;
            
            let stx = crate::node::db_common::StoredTransaction::from_bytes(&raw)
                .map_err(|e| (-32602, format!("deserialization failed: {e}")))?;
            
            {
                let mut pool = state.mempool.lock().await;
                pool.add_transaction(stx.0.clone()).map_err(|e| (-32603, format!("mempool rejected: {e}")))?;
            }

            // Broadcast to P2P network
            let _ = state.p2p_tx.send(crate::net::node::P2pCommand::Broadcast(
                crate::net::protocol::NetworkMessage::Tx(raw)
            ));

            Ok(json!(hex::encode(crate::net::mempool::Mempool::compute_txid_from_stored(&stx.0))))
        }

        "wallet_send" => {
            let mnemonic = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "mnemonic required".to_string()))?;
            ensure_single_wallet_identity(state, mnemonic).await?;
            let recipient_str = params.get(1).and_then(|v| v.as_str()).ok_or((-32602, "recipient required".to_string()))?;
            let amount_kot = params.get(2).and_then(|v| v.as_f64()).ok_or((-32602, "amount required".to_string()))?;
            let gov_data_hex = params.get(3).and_then(|v| v.as_str());

            // 1. Derive Keys
            let (pk, sk) = cached_keypair_for_mnemonic(state, mnemonic).await;
            let sender_addr = crate::crypto::keys::derive_address(&pk);

            // 2. Resolve Recipient
            let recipient_addr = crate::crypto::keys::decode_address_string(recipient_str)
                .map_err(|e| (-32602, format!("invalid recipient: {e}")))?;

            // 2.1 Allow send-to-self for nonce bumping / canceling stuck TX (like ETH)
            // Self-transactions are valid - they just update nonce and pay fee
            // 3. Get Nonce & Balance
            let acc = state.db.get_account(&sender_addr).map_err(|e| (-32603, format!("db error: {e}")))?;
            let amount_knots = (amount_kot * 1e8) as u64;
            
            if acc.balance < amount_knots + 1 { // 1 knot min fee
                return Err((-32603, "insufficient balance".to_string()));
            }

            let gov_data = if let Some(hex) = gov_data_hex {
                let bytes = hex::decode(hex).map_err(|_| (-32602, "invalid governance data hex".to_string()))?;
                if bytes.len() != 32 { return Err((-32602, "governance data must be 32 bytes".to_string())); }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Some(arr)
            } else {
                None
            };

            // 4. Construct Transaction
            let pending_nonce = state.mempool.lock().await.highest_pending_nonce_for_sender(&sender_addr);
            let next_nonce = pending_nonce.unwrap_or(acc.nonce).max(acc.nonce) + 1;

            let mut tx = crate::primitives::transaction::Transaction {
                version: 1,
                sender_address: sender_addr,
                sender_pubkey: pk,
                recipient_address: recipient_addr,
                amount: amount_knots,
                fee: 1, // Minimum fee
                nonce: next_nonce,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                referrer_address: None,
                governance_data: gov_data,
                signature: crate::crypto::dilithium::Signature([0u8; 3309]),
            };

            // 5. Sign
            let hash = tx.signing_hash();
            tx.signature = crate::crypto::dilithium::sign(&hash, &sk);

            // 6. Push to Mempool & Broadcast
            let stx = crate::node::db_common::StoredTransaction {
                version: tx.version,
                sender_address: tx.sender_address,
                sender_pubkey: tx.sender_pubkey.0.to_vec(),
                recipient_address: tx.recipient_address,
                amount: tx.amount,
                fee: tx.fee,
                nonce: tx.nonce,
                timestamp: tx.timestamp,
                referrer_address: tx.referrer_address,
                governance_data: tx.governance_data,
                signature: tx.signature.0.to_vec(),
            };
            let raw = stx.to_bytes();
            {
                let mut pool = state.mempool.lock().await;
                pool.add_transaction(stx).map_err(|e| (-32603, format!("mempool rejected: {e}")))?;
            }

            let _ = state.p2p_tx.send(crate::net::node::P2pCommand::Broadcast(
                crate::net::protocol::NetworkMessage::Tx(raw)
            ));

            Ok(json!({
                "txid": hex::encode(tx.txid()),
                "nonce": tx.nonce,
                "fee": tx.fee
            }))
        }

        "wallet_register_referral" => {
            let mnemonic = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "mnemonic required".to_string()))?;
            ensure_single_wallet_identity(state, mnemonic).await?;
            let referrer_str = params.get(1).and_then(|v| v.as_str()).ok_or((-32602, "referrer required".to_string()))?;

            let (pk, sk) = cached_keypair_for_mnemonic(state, mnemonic).await;
            let sender_addr = crate::crypto::keys::derive_address(&pk);
            let mut s = referrer_str.trim();
            if s.to_uppercase().starts_with("KOT") {
                s = if s.to_uppercase().starts_with("KOT1") {
                    &s[4..]
                } else {
                    &s[3..]
                };
            }

            let referrer_addr = if s.len() == 16 {
                let code = hex::decode(s).map_err(|_| (-32602, "invalid referral code".to_string()))?;
                if code.len() != 8 {
                    return Err((-32602, "invalid referral code".to_string()));
                }
                let mut c = [0u8; 8];
                c.copy_from_slice(&code);
                state.db
                    .get_address_by_referral_code(&c)
                    .map_err(|e| (-32603, format!("db error: {e}")))?
                    .ok_or((-32602, "unknown referral code".to_string()))?
            } else {
                crate::crypto::keys::decode_address_string(referrer_str)
                    .map_err(|e| (-32602, format!("invalid referrer: {e}")))?
            };

            let acc = state.db.get_account(&sender_addr).map_err(|e| (-32603, format!("db error: {e}")))?;
            
            if acc.nonce != 0 {
                return Err((-32603, "wallet already active, referral must be first tx".to_string()));
            }

            if acc.balance < 1 {
                return Err((-32603, "insufficient balance for 1 knot fee".to_string()));
            }

            let mut tx = crate::primitives::transaction::Transaction {
                version: 1,
                sender_address: sender_addr,
                sender_pubkey: pk,
                recipient_address: sender_addr, // send zero to self
                amount: 0,
                fee: 1, // Minimum fee
                nonce: 1, // Must be exactly 1 to trigger state.rs referrer registration
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                referrer_address: Some(referrer_addr),
                governance_data: None,
                signature: crate::crypto::dilithium::Signature([0u8; 3309]),
            };

            let hash = tx.signing_hash();
            tx.signature = crate::crypto::dilithium::sign(&hash, &sk);

            let stx = crate::node::db_common::StoredTransaction {
                version: tx.version,
                sender_address: tx.sender_address,
                sender_pubkey: tx.sender_pubkey.0.to_vec(),
                recipient_address: tx.recipient_address,
                amount: tx.amount,
                fee: tx.fee,
                nonce: tx.nonce,
                timestamp: tx.timestamp,
                referrer_address: tx.referrer_address,
                governance_data: tx.governance_data,
                signature: tx.signature.0.to_vec(),
            };
            
            let raw = stx.to_bytes();
            {
                let mut pool = state.mempool.lock().await;
                pool.add_transaction(stx).map_err(|e| (-32603, format!("mempool rejected: {e}")))?;
            }

            let _ = state.p2p_tx.send(crate::net::node::P2pCommand::Broadcast(
                crate::net::protocol::NetworkMessage::Tx(raw)
            ));

            Ok(json!({
                "txid": hex::encode(tx.txid()),
                "status": "referral_registered"
            }))
        }

        "generatetoaddress" => {
            let count = params.get(0).and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            if count == 0 || count > 500 {
                return Err((-32602, "count must be between 1 and 500".to_string()));
            }

            let addr_str = params.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let miner = if let Ok(a) = crate::crypto::keys::decode_address_string(addr_str) {
                a
            } else {
                let hex_part = if addr_str.to_lowercase().starts_with("kot1") {
                    &addr_str[4..]
                } else if addr_str.to_lowercase().starts_with("kot") {
                    &addr_str[3..]
                } else {
                    addr_str
                };

                match hex::decode(hex_part) {
                    Ok(b) if b.len() == 32 => {
                        let mut a = [0u8; 32];
                        a.copy_from_slice(&b);
                        a
                    }
                    _ => return Err((-32602, "invalid miner address".to_string())),
                }
            };

            let referrer = params.get(2).and_then(|v| v.as_str()).and_then(|mut s| {
                if s.to_uppercase().starts_with("KOT") {
                    s = if s.to_uppercase().starts_with("KOT1") {
                        &s[4..]
                    } else {
                        &s[3..]
                    };
                }

                if s.len() == 16 {
                    let code = hex::decode(s).ok()?;
                    if code.len() == 8 {
                        let mut c = [0u8; 8];
                        c.copy_from_slice(&code);
                        return state.db.get_address_by_referral_code(&c).ok().flatten();
                    }
                } else if s.len() == 64 {
                    let bytes = hex::decode(s).ok()?;
                    if bytes.len() == 32 {
                        let mut r = [0u8; 32];
                        r.copy_from_slice(&bytes);
                        return Some(r);
                    }
                }
                None
            });

            // Thread count: param[3], capped at 8 for fairness
            let thread_count = params.get(3)
                .and_then(|v| v.as_u64())
                .unwrap_or(4)
                .clamp(1, 8) as usize;

            let mut hashes = Vec::new();
            for _ in 0..count {
                let txs = state.mempool.lock().await.get_top_transactions(crate::miner::miner::MAX_TXS);
                let db_clone = state.db.clone();
                let stop_flag = std::sync::atomic::AtomicBool::new(false);
                let miner_clone = miner;
                
                let result = tokio::task::spawn_blocking(move || {
                    crate::miner::miner::mine_block_parallel(
                        &db_clone,
                        txs,
                        &miner_clone,
                        None,
                        &stop_flag,
                        referrer,
                        thread_count,
                    )
                }).await.map_err(|e| (-32603, format!("blocking task error: {}", e)))?;

                if let Some((block, hash)) = result
                    && crate::consensus::state::apply_block(&state.db, &block).is_ok() {
                    // Remove confirmed txs from mempool to avoid stale sender+nonce entries.
                    // This also prevents Replace-by-Fee checks from rejecting subsequent txs.
                    let confirmed: Vec<[u8; 32]> = block
                        .tx_data
                        .iter()
                        .map(crate::net::mempool::Mempool::compute_txid_from_stored)
                        .collect();
                    state.mempool.lock().await.remove_confirmed(&confirmed);
                    hashes.push(hex::encode(hash));
                }
            }
            Ok(json!(hashes))
        }

        "getreferralinfo" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let addr = if let Ok(a) = crate::crypto::keys::decode_address_string(addr_str) {
                a
            } else {
                let hex_part = if addr_str.to_lowercase().starts_with("kot1") {
                    &addr_str[4..]
                } else if addr_str.to_lowercase().starts_with("kot") {
                    &addr_str[3..]
                } else {
                    addr_str
                };
                match hex::decode(hex_part) {
                    Ok(b) if b.len() == 32 => {
                        let mut a = [0u8; 32];
                        a.copy_from_slice(&b);
                        a
                    }
                    _ => return Err((-32602, "invalid address".to_string())),
                }
            };

            match state.db.get_account(&addr) {
                Ok(a) => {
                    let code = crate::crypto::hash::hash_sha3_256(&addr);
                    let is_active = a.total_referred_miners > 0
                        && a.last_mined_height > 0 
                        && state.db.get_chain_height().unwrap_or(0) as u64 - a.last_mined_height <= 2880;
                    Ok(json!({
                        "privacy_code":                 hex::encode(&code[..8]),
                        "referred_by":                  a.referrer.map(|r| {
                            let ref_code = crate::crypto::hash::hash_sha3_256(&r);
                            hex::encode(&ref_code[..8])
                        }),
                        "total_referred_miners":        a.total_referred_miners,
                        "total_referral_bonus_earned":  a.total_referral_bonus_earned,
                        "total_referral_bonus_kot":     format!("{:.8}", a.total_referral_bonus_earned as f64 / 1e8),
                        "is_active_referrer":           is_active,
                        "governance_weight":            a.governance_weight,
                    }))
                }
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "getgovernanceinfo" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let addr = if let Ok(a) = crate::crypto::keys::decode_address_string(addr_str) {
                a
            } else {
                let hex_part = if addr_str.to_lowercase().starts_with("kot1") {
                    &addr_str[4..]
                } else if addr_str.to_lowercase().starts_with("kot") {
                    &addr_str[3..]
                } else {
                    addr_str
                };
                match hex::decode(hex_part) {
                    Ok(b) if b.len() == 32 => {
                        let mut a = [0u8; 32];
                        a.copy_from_slice(&b);
                        a
                    }
                    _ => return Err((-32602, "invalid address".to_string())),
                }
            };

            match state.db.get_account(&addr) {
                Ok(a) => {
                    let weight_bps = a.governance_weight;
                    // Get current cap from state (dynamic, not hardcoded)
                    let cap_bps = state.db.get_governance_params()
                        .map(|p| p.cap_bps)
                        .unwrap_or(1000);
                    let is_capped = weight_bps >= cap_bps;
                    Ok(json!({
                        "address":                crate::crypto::keys::encode_address_string(&addr),
                        "governance_weight":      a.governance_weight,
                        "governance_weight_bps":  weight_bps,
                        "governance_weight_pct":  format!("{:.2}%", weight_bps as f64 / 100.0),
                        "cap_bps":                cap_bps,
                        "cap_pct":                format!("{:.2}%", cap_bps as f64 / 100.0),
                        "is_capped":              is_capped,
                    }))
                }
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "getgovernancetally" => {
            let prop_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let prop_hash = hex::decode(prop_str)
                .map_err(|_| (-32602, "invalid proposal hash".to_string()))?;
            if prop_hash.len() != 32 {
                return Err((-32602, "proposal hash must be 32 bytes".to_string()));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&prop_hash);

            match state.db.get_governance_tally(&hash) {
                Ok(tally) => {
                    let is_passed = tally >= 5100;
                    Ok(json!({
                        "proposal_hash":       hex::encode(hash),
                        "total_weight_bps":    tally,
                        "total_weight_pct":    format!("{:.2}%", tally as f64 / 100.0),
                        "threshold_bps":       5100,
                        "threshold_pct":       "51.0%",
                        "is_passed":           is_passed,
                    }))
                }
                Err(e) => Err((-32603, format!("db error: {e}"))),
            }
        }

        "get_all_miners" => {
            // Cache miners data for 5 seconds to reduce DB load (scanning blockchain is expensive)
            static MINERS_CACHE: std::sync::OnceLock<std::sync::Mutex<(serde_json::Value, u64)>> = std::sync::OnceLock::new();
            let cache = MINERS_CACHE.get_or_init(|| std::sync::Mutex::new((json!({}), 0)));
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let cached_result = {
                let cache_guard = cache.lock().unwrap();
                if now - cache_guard.1 < 5 && !cache_guard.0.is_null() {
                    Some(cache_guard.0.clone())
                } else {
                    None
                }
            };
            
            if let Some(cached) = cached_result {
                return Ok(cached);
            }

            // SCAN ACTUAL BLOCKCHAIN to compute accurate blocks per miner
            let chain_height = state.db.get_chain_height().unwrap_or(0);
            let mut miner_blocks: std::collections::HashMap<[u8; 32], u64> = std::collections::HashMap::new();
            let mut miner_last_height: std::collections::HashMap<[u8; 32], u32> = std::collections::HashMap::new();
            let mut miner_rewards: std::collections::HashMap<[u8; 32], u64> = std::collections::HashMap::new();
            
            // Scan all blocks to count actual blocks per miner
            for h in 1..=chain_height {
                if let Ok(Some(hash)) = state.db.get_block_hash_by_height(h) {
                    if let Ok(Some(block)) = state.db.get_block(&hash) {
                        let miner = block.miner_address;
                        *miner_blocks.entry(miner).or_insert(0) += 1;
                        miner_last_height.insert(miner, h);
                        let reward = crate::consensus::chain::calculate_block_reward(h as u64);
                        *miner_rewards.entry(miner).or_insert(0) += reward;
                    }
                }
            }

            // Get current mining address
            let current_mining_addr = state.mining_address.lock().await.clone();
            let is_mining_active = state.mining_active.load(Ordering::SeqCst);

            // Build miners list from actual blockchain data
            let mut miners = Vec::new();
            for (addr, blocks_count) in &miner_blocks {
                let addr_str = crate::crypto::keys::encode_address_string(addr);
                let last_h = miner_last_height.get(addr).copied().unwrap_or(0);
                
                // Get balance from account state
                let acc = state.db.get_account(addr).unwrap_or_default();
                let referrer_str = acc.referrer.map(|r| crate::crypto::keys::encode_address_string(&r));
                
                // Get timestamp from last mined block
                let last_block_time = if last_h > 0 {
                    match state.db.get_block_hash_by_height(last_h) {
                        Ok(Some(hash)) => {
                            match state.db.get_block(&hash) {
                                Ok(Some(block)) => {
                                    let ts = u32::from_le_bytes(block.timestamp);
                                    (ts as u64) * 1000
                                }
                                _ => now * 1000,
                            }
                        }
                        _ => now * 1000,
                    }
                } else {
                    now * 1000
                };

                let is_currently_mining = is_mining_active && current_mining_addr.as_ref() == Some(addr);
                
                // Calculate total rewards from consensus schedule
                let total_reward_knots = *miner_rewards.get(addr).unwrap_or(&0);
                let total_reward_kot = format!("{:.2}", total_reward_knots as f64 / 1e8);

                miners.push(json!({
                    "address": addr_str,
                    "blocks_mined": blocks_count,
                    "last_mined_height": last_h,
                    "balance_knots": acc.balance,
                    "balance_kot": format!("{:.8}", acc.balance as f64 / 1e8),
                    "total_reward_kot": total_reward_kot,
                    "nonce": acc.nonce,
                    "referrer": referrer_str,
                    "last_block_time": last_block_time,
                    "is_mining": is_currently_mining,
                }));
            }

            // Sort by blocks mined descending
            miners.sort_by(|a, b| {
                let ba = a.get("blocks_mined").and_then(|v| v.as_u64()).unwrap_or(0);
                let bb = b.get("blocks_mined").and_then(|v| v.as_u64()).unwrap_or(0);
                bb.cmp(&ba)
            });

            let result = json!({
                "miners": miners,
                "chain_height": chain_height,
                "total_miners": miner_blocks.len(),
            });

            {
                let mut cache_guard = cache.lock().unwrap();
                cache_guard.0 = result.clone();
                cache_guard.1 = now;
            }

            Ok(result)
        }

        "estimatefee" => {
            let tx_size = params.get(0).and_then(|v| v.as_u64()).unwrap_or(5400) as u64;
            let pool = state.mempool.lock().await;
            let pool_size = pool.size();
            let base_fee = 1u64;
            let congestion_fee = if pool_size > 10 {
                (pool_size as u64 - 10) / 3
            } else {
                0
            };
            let recommended = base_fee + congestion_fee;
            let fast = recommended + (recommended / 2).max(1);
            Ok(json!({
                "recommended_fee_knots": recommended,
                "fast_fee_knots": fast,
                "tx_size_bytes": tx_size,
                "mempool_size": pool_size,
            }))
        }

        "gettransactionhistory" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            let addr = if let Ok(a) = crate::crypto::keys::decode_address_string(addr_str) {
                a
            } else {
                return Err((-32602, "invalid address".to_string()));
            };
            let limit = params.get(1).and_then(|v| v.as_u64()).unwrap_or(50).min(200) as u32;

            let chain_height = state.db.get_chain_height().map_err(|e| (-32603, format!("db error: {e}")))?;
            let mut txs = Vec::new();
            let scan_depth = limit * 20;
            let start = chain_height;
            let end = chain_height.saturating_sub(scan_depth);

            for h in (end..=start).rev() {
                if txs.len() >= limit as usize { break; }
                let hash = match state.db.get_block_hash_by_height(h) {
                    Ok(Some(hash)) => hash,
                    _ => continue,
                };
                let block = match state.db.get_block(&hash) {
                    Ok(Some(b)) => b,
                    _ => continue,
                };
                let block_height = u32::from_le_bytes(block.block_height);
                let block_time = u32::from_le_bytes(block.timestamp);

                if block.miner_address == addr {
                    let reward = crate::consensus::chain::calculate_block_reward(block_height as u64);
                    txs.push(json!({
                        "type": "mining_reward",
                        "address": crate::crypto::keys::encode_address_string(&block.miner_address),
                        "amount_knots": reward,
                        "amount_kot": format!("{:.8}", reward as f64 / 1e8),
                        "fee_knots": 0,
                        "block_height": block_height,
                        "timestamp": block_time,
                    }));
                }

                for tx in &block.tx_data {
                    if tx.sender_address == addr {
                        txs.push(json!({
                            "type": "sent",
                            "address": crate::crypto::keys::encode_address_string(&tx.recipient_address),
                            "amount_knots": tx.amount,
                            "amount_kot": format!("{:.8}", tx.amount as f64 / 1e8),
                            "fee_knots": tx.fee,
                            "block_height": block_height,
                            "timestamp": block_time,
                            "nonce": tx.nonce,
                        }));
                    } else if tx.recipient_address == addr {
                        txs.push(json!({
                            "type": "received",
                            "address": crate::crypto::keys::encode_address_string(&tx.sender_address),
                            "amount_knots": tx.amount,
                            "amount_kot": format!("{:.8}", tx.amount as f64 / 1e8),
                            "fee_knots": tx.fee,
                            "block_height": block_height,
                            "timestamp": block_time,
                            "nonce": tx.nonce,
                        }));
                    }
                }
            }

            Ok(json!({
                "address": addr_str,
                "transactions": txs,
                "count": txs.len(),
            }))
        }

        "addnode" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "address required".to_string()))?;
            let addr: SocketAddr = addr_str.parse().map_err(|_| (-32602, "invalid socket address".to_string()))?;
            state.p2p_tx.send(P2pCommand::Connect(addr)).map_err(|_| (-32603, "internal error".to_string()))?;
            Ok(json!("added"))
        }

        "wallet_create" => {
            // Single-wallet-per-profile: don't create a second wallet in the same data dir.
            if wallet_keys_file(&state.data_dir).exists() {
                return Err((-32603, "wallet already initialized in this profile".to_string()));
            }
            let mnemonic = crate::crypto::keys::generate_mnemonic();
            let (pk, _sk) = cached_keypair_for_mnemonic(state, &mnemonic).await;
            let addr = crate::crypto::keys::derive_address(&pk);
            let addr_str = crate::crypto::keys::encode_address_string(&addr);
            Ok(json!({
                "mnemonic": mnemonic,
                "address": addr_str,
            }))
        }

        "wallet_get_address" => {
            let mnemonic = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "mnemonic required".to_string()))?;
            ensure_single_wallet_identity(state, mnemonic).await?;
            let (pk, _sk) = cached_keypair_for_mnemonic(state, mnemonic).await;
            let addr = crate::crypto::keys::derive_address(&pk);
            let addr_str = crate::crypto::keys::encode_address_string(&addr);
            Ok(json!({
                "address": addr_str,
            }))
        }

        "wallet_create_file" => {
            // Creates wallet.dat file with deterministic address storage
            let mnemonic = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "mnemonic required".to_string()))?;
            let password = params.get(1).and_then(|v| v.as_str()).ok_or((-32602, "password required".to_string()))?;
            let wallet_path = params.get(2).and_then(|v| v.as_str()).unwrap_or("~/.knotcoin/mainnet/wallet.dat");
            
            // Expand ~ to home directory
            let expanded_path = if wallet_path.starts_with("~/") {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                wallet_path.replacen("~", &home, 1)
            } else {
                wallet_path.to_string()
            };
            
            // Create wallet file
            let wallet_file = crate::wallet::file::WalletFile::create_from_mnemonic(mnemonic, password)
                .map_err(|e| (-32603, format!("failed to create wallet: {}", e)))?;
            
            // Save to disk
            wallet_file.save(&expanded_path)
                .map_err(|e| (-32603, format!("failed to save wallet: {}", e)))?;
            
            Ok(json!({
                "address": wallet_file.address,
                "path": expanded_path,
                "created": wallet_file.created,
                "mnemonic_hint": wallet_file.mnemonic_hint,
            }))
        }

        "wallet_unlock_file" => {
            // Unlocks wallet.dat file and returns address
            let password = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "password required".to_string()))?;
            let wallet_path = params.get(1).and_then(|v| v.as_str()).unwrap_or("~/.knotcoin/mainnet/wallet.dat");
            
            // Expand ~ to home directory
            let expanded_path = if wallet_path.starts_with("~/") {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                wallet_path.replacen("~", &home, 1)
            } else {
                wallet_path.to_string()
            };
            
            // Load wallet file
            let wallet_file = crate::wallet::file::WalletFile::load(&expanded_path)
                .map_err(|e| (-32603, format!("failed to load wallet: {}", e)))?;
            
            // Verify password by attempting to decrypt
            wallet_file.decrypt_secret_key(password)
                .map_err(|e| (-32603, format!("failed to unlock wallet: {}", e)))?;
            
            Ok(json!({
                "address": wallet_file.address,
                "created": wallet_file.created,
                "mnemonic_hint": wallet_file.mnemonic_hint,
            }))
        }

        "wallet_reset" => {
            // Backup wallet_keys.json before deletion (allows recovery with same mnemonic)
            let wallet_path = wallet_keys_file(&state.data_dir);
            if wallet_path.exists() {
                let backup_path = wallet_path.with_extension("json.backup");
                // Keep backup so same mnemonic can restore same address
                let _ = std::fs::copy(&wallet_path, &backup_path);
                std::fs::remove_file(&wallet_path).map_err(|e| (-32603, format!("Failed to delete wallet file: {}", e)))?;
            }
            state.wallet_keys.lock().await.clear();
            Ok(json!({ "result": "wallet reset", "note": "Keys backed up to wallet_keys.json.backup" }))
        }
        "start_mining" => {
            let mnemonic = params.get(0).and_then(|v| v.as_str())
                .ok_or((-32602, "mnemonic required".to_string()))?;
            ensure_single_wallet_identity(state, mnemonic).await?;
            let threads = params.get(1).and_then(|v| v.as_u64()).unwrap_or(2).clamp(1, 8) as usize;
            let referrer_str = params.get(2).and_then(|v| v.as_str());

            if state.mining_active.load(Ordering::SeqCst) {
                return Ok(json!({ "status": "already_mining" }));
            }

            let (pk, _sk) = cached_keypair_for_mnemonic(state, mnemonic).await;
            let miner_addr = crate::crypto::keys::derive_address(&pk);
            
            let referrer = if let Some(r) = referrer_str {
                let mut s = r.trim();
                if s.to_uppercase().starts_with("KOT") {
                    s = if s.to_uppercase().starts_with("KOT1") {
                        &s[4..]
                    } else {
                        &s[3..]
                    };
                }

                if s.len() == 16 {
                    let code = match hex::decode(s) {
                        Ok(c) => c,
                        Err(_) => Vec::new(),
                    };
                    if code.len() == 8 {
                        let mut c = [0u8; 8];
                        c.copy_from_slice(&code);
                        state.db.get_address_by_referral_code(&c).ok().flatten()
                    } else {
                        None
                    }
                } else {
                    crate::crypto::keys::decode_address_string(r).ok()
                }
            } else {
                None
            };

            state.mining_active.store(true, Ordering::SeqCst);
            state.mining_blocks_found.store(0, Ordering::SeqCst);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            state.mining_start_time.store(now, Ordering::SeqCst);
            *state.mining_address.lock().await = Some(miner_addr);
            *state.mining_referrer.lock().await = referrer;

            let db = state.db.clone();
            let mempool = state.mempool.clone();
            let p2p_tx = state.p2p_tx.clone();
            let mining_active_ref = state.mining_stop.clone();
            mining_active_ref.store(false, Ordering::SeqCst);
            let blocks_counter = state.mining_blocks_found.clone();

            let addr_copy = miner_addr;
            let referrer_copy = referrer;
            let stop_flag = state.mining_stop.clone();
            let nonce_counter = state.mining_nonces_total.clone();
            tokio::spawn(async move {
                println!("[miner] Background mining started ({} threads)", threads);
                loop {
                    if stop_flag.load(Ordering::SeqCst) {
                        println!("[miner] Mining stopped by user");
                        break;
                    }

                    let txs = mempool.lock().await.get_top_transactions(crate::miner::miner::MAX_TXS);
                    
                    let db_clone = db.clone();
                    let inner_stop = stop_flag.clone();
                    let nonce_counter_clone = nonce_counter.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        crate::miner::miner::mine_block_parallel_with_counter(
                            &db_clone, txs, &addr_copy, None, &inner_stop, referrer_copy, threads,
                            Some(&nonce_counter_clone),
                        )
                    }).await.unwrap_or(None);

                    if let Some((block, hash)) = result {
                        if crate::consensus::state::apply_block_with_referrer(&db, &block, referrer_copy).is_ok() {
                            // Remove confirmed txs from mempool so we don't keep stale sender+nonce entries.
                            let confirmed: Vec<[u8; 32]> = block
                                .tx_data
                                .iter()
                                .map(crate::net::mempool::Mempool::compute_txid_from_stored)
                                .collect();
                            mempool.lock().await.remove_confirmed(&confirmed);
                            blocks_counter.fetch_add(1, Ordering::SeqCst);
                            println!("[miner] Block found: {}", hex::encode(&hash));
                            let block_bytes = block.to_bytes();
                            let _ = p2p_tx.send(crate::net::node::P2pCommand::Broadcast(
                                crate::net::protocol::NetworkMessage::Blocks(vec![block_bytes])
                            ));
                            
                            // Yield to tokio and other tasks (e.g., P2P network, RPC) to avoid node starvation when difficulty is very low
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            });

            Ok(json!({
                "status": "mining_started",
                "threads": threads,
                "address": crate::crypto::keys::encode_address_string(&miner_addr),
            }))
        }

        "stop_mining" => {
            state.mining_stop.store(true, Ordering::SeqCst);
            state.mining_active.store(false, Ordering::SeqCst);
            *state.mining_address.lock().await = None;
            Ok(json!({ "status": "mining_stopped" }))
        }

        "get_mining_status" => {
            let active = state.mining_active.load(Ordering::SeqCst);
            let blocks = state.mining_blocks_found.load(Ordering::SeqCst);
            let start = state.mining_start_time.load(Ordering::SeqCst);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            let uptime = if active && start > 0 { now - start } else { 0 };
            let nonces = state.mining_nonces_total.load(Ordering::SeqCst);
            let hashrate = if uptime > 0 { nonces / uptime } else { 0 };
            
            // Get difficulty from latest block
            let chain_height = state.db.get_chain_height().unwrap_or(0);
            let difficulty_bits = if chain_height > 0 {
                if let Ok(Some(hash)) = state.db.get_block_hash_by_height(chain_height) {
                    if let Ok(Some(block)) = state.db.get_block(&hash) {
                        let target = &block.difficulty_target;
                        // Count leading zero bytes (not 0xff) for difficulty
                        let mut leading_zeros = 0u32;
                        for &b in target.iter() {
                            if b == 0x00 { leading_zeros += 8; }
                            else { leading_zeros += b.leading_zeros() as u32; break; }
                        }
                        // Minimum difficulty is 1
                        leading_zeros.max(1)
                    } else { 1 }
                } else { 1 }
            } else { 1 };
            
            Ok(json!({
                "active": active,
                "blocks_found": blocks,
                "uptime_seconds": uptime,
                "hashrate": hashrate,
                "nonces_total": nonces,
                "difficulty_bits": difficulty_bits,
                "chain_height": chain_height,
            }))
        }

        "getpeerinfo" => {
            let count = state.connected_peers.load(Ordering::Relaxed);
            let known = load_known_peers_from_disk(&state.data_dir);
            Ok(json!({
                "connected": count > 0,
                "peer_count": count,
                "known_peers": known.len(),
                "known_peers_sample": known.into_iter().take(16).collect::<Vec<_>>(),
            }))
        }

        "getaddressstats" => {
            // Cache for 10 seconds to avoid heavy scans under load
            use std::sync::{Mutex, OnceLock};
            static CACHE: OnceLock<Mutex<(serde_json::Value, u64)>> = OnceLock::new();
            let cache = CACHE.get_or_init(|| Mutex::new((json!({}), 0)));

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            {
                let guard = cache.lock().unwrap();
                if now - guard.1 < 10 && !guard.0.is_null() {
                    return Ok(guard.0.clone());
                }
            }

            let mut total = 0u64;
            let mut nonzero = 0u64;
            let mut total_balance = 0u64;
            if let Ok(accounts) = state.db.iter_accounts() {
                for (_addr, acc) in accounts {
                    total += 1;
                    total_balance = total_balance.saturating_add(acc.balance);
                    if acc.balance > 0 {
                        nonzero += 1;
                    }
                }
            }

            let result = json!({
                "total_accounts": total,
                "nonzero_accounts": nonzero,
                "total_balance_knots": total_balance,
                "total_balance_kot": format!("{:.8}", total_balance as f64 / 1e8),
            });

            let mut guard = cache.lock().unwrap();
            *guard = (result.clone(), now);
            Ok(result)
        }

        "getnetworkhashrate" => {
            let chain_height = state.db.get_chain_height().unwrap_or(0);
            let hashrate = if chain_height > 0 {
                if let Ok(Some(hash)) = state.db.get_block_hash_by_height(chain_height) {
                    if let Ok(Some(block)) = state.db.get_block(&hash) {
                        estimate_network_hashrate_from_target(&block.difficulty_target)
                    } else { 0 }
                } else { 0 }
            } else { 0 };

            Ok(json!({
                "hashrate": hashrate,
                "unit": "H/s"
            }))
        }

        "stop" => {
            state.shutdown.store(true, Ordering::SeqCst);
            Ok(json!("stopping"))
        }

        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

async fn handle_request(
    state: Arc<RpcState>,
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if req.method() == hyper::Method::OPTIONS {
        let builder = Response::builder()
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type, Authorization");
        return Ok(builder.body(Full::new(Bytes::new())).unwrap());
    }

    // SECURITY FIX: Verify bearer token authentication
    // Protects against SSRF and DNS rebinding attacks from malicious browser JavaScript
    let auth_header = req.headers().get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    
    if !auth_header.starts_with("Bearer ") || auth_header[7..] != state.auth_token {
        let builder = Response::builder()
            .status(hyper::StatusCode::UNAUTHORIZED)
            .header("Access-Control-Allow-Origin", "*");
        return Ok(builder.body(Full::new(Bytes::from("Unauthorized"))).unwrap());
    }

    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            let mut res = Response::new(Full::new(Bytes::from("Bad Request")));
            *res.status_mut() = hyper::StatusCode::BAD_REQUEST;
            return Ok(res);
        }
    };

    let resp = match serde_json::from_slice::<Value>(&body) {
        Ok(v) => {
            let id = v.get("id").cloned().unwrap_or(json!(null));
            if !v.is_object() || v.get("method").is_none() {
                json!({
                    "jsonrpc": "2.0",
                    "error": {"code": -32600, "message": "Invalid Request"},
                    "id": id
                })
            } else {
                let method = v["method"].as_str().unwrap_or("");
                let params = v.get("params").cloned().unwrap_or(json!([]));
                match handle_rpc(&state, method, &params).await {
                    Ok(result) => json!({ "jsonrpc": "2.0", "result": result, "id": id }),
                    Err((code, message)) => json!({
                        "jsonrpc": "2.0",
                        "error": {"code": code, "message": message},
                        "id": id
                    }),
                }
            }
        }
        Err(e) => json!({
            "jsonrpc": "2.0",
            "error": {"code": -32700, "message": format!("parse error: {e}")},
            "id": null,
        }),
    };

    let body_bytes = serde_json::to_vec(&resp).unwrap();
    let builder = Response::builder()
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type, Authorization");

    Ok(builder.body(Full::new(Bytes::from(body_bytes))).unwrap())
}

pub async fn start_rpc_server(
    state: Arc<RpcState>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr: SocketAddr = format!("{RPC_BIND_ADDRESS}:{port}").parse()?;
    let listener = TcpListener::bind(addr).await?;

    loop {
        if state.shutdown.load(Ordering::SeqCst) { break; }
        let (stream, _) = match timeout(Duration::from_millis(250), listener.accept()).await {
            Ok(Ok(pair)) => pair,
            _ => continue,
        };
        let s = state.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req| {
                let s2 = s.clone();
                async move { handle_request(s2, req).await }
            });
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
    Ok(())
}
/// Generate or load RPC authentication token
/// SECURITY: Creates a high-entropy bearer token to prevent SSRF/DNS rebinding attacks
pub fn generate_rpc_auth_token(data_dir: &str) -> Result<String, std::io::Error> {
    use std::fs;
    use std::path::Path;

    let cookie_path = Path::new(data_dir).join(RPC_COOKIE_FILE);

    // Try to read existing cookie
    if let Ok(token) = fs::read_to_string(&cookie_path) {
        let token = token.trim();
        if token.len() >= 32 {
            return Ok(token.to_string());
        }
    }

    // Generate new high-entropy token (32 bytes = 64 hex chars)
    use crate::crypto::hash::hash_sha3_256;
    let random_bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    let hash = hash_sha3_256(&random_bytes);
    let token = hex::encode(&hash[..32]);

    // Save to cookie file
    fs::write(&cookie_path, &token)?;

    // Set restrictive permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&cookie_path)?.permissions();
        perms.set_mode(0o600); // Read/write for owner only
        fs::set_permissions(&cookie_path, perms)?;
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::estimate_network_hashrate_from_target;

    #[test]
    fn test_hashrate_no_overflow_on_max_target() {
        let target = [0xffu8; 32];
        let h = estimate_network_hashrate_from_target(&target);
        assert_eq!(h, 0);
    }

    #[test]
    fn test_hashrate_zero_target_is_safe() {
        let target = [0u8; 32];
        let h = estimate_network_hashrate_from_target(&target);
        assert!(h > 0);
    }
}
