use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

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

pub struct RpcState {
    pub db: ChainDB,
    pub mempool: Arc<Mutex<Mempool>>,
    pub shutdown: AtomicBool,
    pub p2p_tx: tokio::sync::mpsc::UnboundedSender<P2pCommand>,
    pub auth_token: String, // SECURITY: Bearer token for RPC authentication
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

            Ok(json!({
                "blocks":      height,
                "difficulty":  difficulty,
                "mempool":     pool_size,
                "network":     "mainnet",
                "quantum_sec": "Dilithium3 (NIST FIPS 204)",
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

            let mut hashes = Vec::new();
            for _ in 0..count {
                let mut pool = state.mempool.lock().await;
                if let Some((block, hash)) = crate::miner::miner::mine_block(
                    &state.db,
                    &mut pool,
                    &miner,
                    None,
                    &state.shutdown,
                    referrer,
                )
                    && crate::consensus::state::apply_block(&state.db, &block).is_ok() {
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
                    let is_active = a.last_mined_height > 0 
                        && state.db.get_chain_height().unwrap_or(0) as u64 - a.last_mined_height <= 2880;
                    Ok(json!({
                        "address":                      crate::crypto::keys::encode_address_string(&addr),
                        "privacy_code":                 hex::encode(&code[..8]),
                        "referred_by":                  a.referrer.map(hex::encode),
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
            // Cache miners data for 2 seconds to reduce DB load
            static MINERS_CACHE: std::sync::OnceLock<std::sync::Mutex<(Vec<serde_json::Value>, u64)>> = std::sync::OnceLock::new();
            let cache = MINERS_CACHE.get_or_init(|| std::sync::Mutex::new((Vec::new(), 0)));
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let cache_guard = cache.lock().unwrap();
            if now - cache_guard.1 < 2 && !cache_guard.0.is_empty() {
                return Ok(json!({ "miners": cache_guard.0.clone() }));
            }
            drop(cache_guard);

            let mut miners = Vec::new();

            // Iterate over all accounts using the new iterator method
            let accounts = match state.db.iter_accounts() {
                Ok(accts) => accts,
                Err(e) => {
                    eprintln!("Failed to iterate accounts: {}", e);
                    return Ok(json!({ "miners": [] }));
                }
            };

            for (addr, acc) in accounts {
                if acc.total_blocks_mined == 0 {
                    continue;
                }

                let addr_str = crate::crypto::keys::encode_address_string(&addr);
                let referrer_str = acc.referrer.map(|r| hex::encode(r));

                let first_block_time = if acc.last_mined_height > 0 {
                    match state.db.get_block_hash_by_height(acc.last_mined_height as u32) {
                        Ok(Some(hash)) => {
                            match state.db.get_block(&hash) {
                                Ok(Some(block)) => {
                                    let ts = u32::from_le_bytes(block.timestamp);
                                    (ts as u64) * 1000
                                }
                                _ => std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            }
                        }
                        _ => std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64,
                    }
                } else {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                };

                miners.push(json!({
                    "address": addr_str,
                    "blocks_mined": acc.total_blocks_mined,
                    "last_mined_height": acc.last_mined_height,
                    "referrer": referrer_str,
                    "joined_at": first_block_time,
                }));
            }

            let mut cache_guard = cache.lock().unwrap();
            cache_guard.0 = miners.clone();
            cache_guard.1 = now;
            drop(cache_guard);

            Ok(json!({ "miners": miners }))
        }

        "addnode" => {
            let addr_str = params.get(0).and_then(|v| v.as_str()).ok_or((-32602, "address required".to_string()))?;
            let addr: SocketAddr = addr_str.parse().map_err(|_| (-32602, "invalid socket address".to_string()))?;
            state.p2p_tx.send(P2pCommand::Connect(addr)).map_err(|_| (-32603, "internal error".to_string()))?;
            Ok(json!("added"))
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
        let mut res = Response::new(Full::new(Bytes::from("Unauthorized")));
        *res.status_mut() = hyper::StatusCode::UNAUTHORIZED;
        return Ok(res);
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
        .header("Access-Control-Allow-Origin", "*");

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


