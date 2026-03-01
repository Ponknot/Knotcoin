// Mining loop: assemble template → initialize PONC scratchpad → iterate nonces.
// A new engine is created per template so the scratchpad is always clean.
//
// FAIRNESS: Mining is hard-capped at 8 threads to prevent hardware arms race.
// This ensures consumer hardware (4-8 cores) can compete with servers.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::consensus::chain::calculate_new_difficulty;
use crate::consensus::state::{apply_block, block_hash};
use crate::crypto::hash::hash_sha3_256;
use crate::crypto::ponc::ffi::bridge::new_ponc_engine;
use crate::net::mempool::Mempool;
use crate::node::{ChainDB, db_common::{StoredBlock, StoredTransaction}};

pub const MAX_TXS: usize = 6;
const RETARGET_INTERVAL: u64 = 60;

// Use shared StoredBlock::header_bytes implementation for PoC/PoW consistency.

fn merkle_root(txs: &[StoredTransaction]) -> [u8; 32] {
    if txs.is_empty() {
        return [0u8; 32];
    }

    let mut hashes: Vec<[u8; 32]> = txs
        .iter()
        .map(|tx| {
            let b = tx.to_bytes();
            // Strip signature for txid computation consistency
            hash_sha3_256(&b)
        })
        .collect();

    while hashes.len() > 1 {
        let mut next = Vec::new();
        for pair in hashes.chunks(2) {
            let mut combined = pair[0].to_vec();
            combined.extend_from_slice(if pair.len() == 2 { &pair[1] } else { &pair[0] });
            next.push(hash_sha3_256(&combined));
        }
        hashes = next;
    }
    hashes[0]
}

// Calculate the difficulty target to use for the next block.
// Reads the actual time taken over the last RETARGET_INTERVAL blocks.
fn next_difficulty(db: &ChainDB, current_height: u32, current_target: [u8; 32]) -> [u8; 32] {
    if current_height == 0 || !(current_height as u64).is_multiple_of(RETARGET_INTERVAL) {
        return current_target;
    }

    let window_start = current_height.saturating_sub(RETARGET_INTERVAL as u32);
    let start_hash = match db.get_block_hash_by_height(window_start) {
        Ok(Some(h)) => h,
        _ => return current_target,
    };
    let start_block = match db.get_block(&start_hash) {
        Ok(Some(b)) => b,
        _ => return current_target,
    };

    let start_ts = u32::from_le_bytes(start_block.timestamp) as u64;
    let tip_hash = match db.get_tip() {
        Ok(Some(h)) => h,
        _ => return current_target,
    };
    let tip_block = match db.get_block(&tip_hash) {
        Ok(Some(b)) => b,
        _ => return current_target,
    };
    let end_ts = u32::from_le_bytes(tip_block.timestamp) as u64;

    let elapsed = end_ts.saturating_sub(start_ts).max(1);
    calculate_new_difficulty(&current_target, elapsed)
}

pub fn mine_block(
    db: &ChainDB,
    txs: Vec<StoredTransaction>,
    miner_addr: &[u8; 32],
    miner_sk: Option<&crate::crypto::dilithium::SecretKey>,
    stop: &AtomicBool,
    referrer: Option<[u8; 32]>,
) -> Option<(StoredBlock, [u8; 32])> {
    // Get thread count from governance params, hard-capped at 8
    let params = db.get_governance_params().unwrap_or_default();
    let num_threads = (params.mining_threads as usize).clamp(1, 8);
    
    mine_block_parallel(db, txs, miner_addr, miner_sk, stop, referrer, num_threads)
}

pub fn mine_block_parallel(
    db: &ChainDB,
    txs: Vec<StoredTransaction>,
    miner_addr: &[u8; 32],
    miner_sk: Option<&crate::crypto::dilithium::SecretKey>,
    stop: &AtomicBool,
    referrer: Option<[u8; 32]>,
    num_threads: usize,
) -> Option<(StoredBlock, [u8; 32])> {
    mine_block_parallel_with_counter(db, txs, miner_addr, miner_sk, stop, referrer, num_threads, None)
}

pub fn mine_block_parallel_with_counter(
    db: &ChainDB,
    txs: Vec<StoredTransaction>,
    miner_addr: &[u8; 32],
    miner_sk: Option<&crate::crypto::dilithium::SecretKey>,
    stop: &AtomicBool,
    referrer: Option<[u8; 32]>,
    num_threads: usize,
    global_nonce_counter: Option<&AtomicU64>,
) -> Option<(StoredBlock, [u8; 32])> {
    let (prev_hash, height, base_target) = match db.get_tip().ok()? {
        Some(h) => {
            let tip = db.get_block(&h).ok()??;
            let ht = u32::from_le_bytes(tip.block_height);
            (h, ht + 1, tip.difficulty_target)
        }
        None => return None, // genesis must be applied before mining
    };

    let difficulty_target = next_difficulty(db, height, base_target);

    let mut now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    // Ensure timestamp is strictly greater than Median-Time-Past (MTP).
    // Without this, rapid block generation (tests, fast networks) can produce
    // blocks with the same timestamp that fail the MTP consensus check.
    {
        let mut times = Vec::new();
        for i in 1..=11u32 {
            if height >= i {
                if let Ok(Some(h)) = db.get_block_hash_by_height(height - i) {
                    if let Ok(Some(b)) = db.get_block(&h) {
                        times.push(u32::from_le_bytes(b.timestamp));
                    }
                }
            }
        }
        if !times.is_empty() {
            times.sort();
            let mtp = times[times.len() / 2];
            if now <= mtp {
                now = mtp + 1;
            }
        }
    }

    // NOTE: Referral binding transactions are NOT auto-inserted by the miner.
    // The miner does not currently have a reliable way to reconstruct the matching Dilithium public
    // key from only a stored secret key (and the chain requires pubkey->address consistency).
    // Referral registration must be performed explicitly via RPC `wallet_register_referral`
    // as the wallet's first outgoing transaction.
    let _ = (referrer, miner_sk);

    let root = merkle_root(&txs);
    let template = StoredBlock {
        version: [1, 0, 0, 0],
        previous_hash: prev_hash,
        merkle_root: root,
        timestamp: now.to_le_bytes(),
        difficulty_target,
        nonce: [0u8; 8],
        block_height: height.to_le_bytes(),
        miner_address: *miner_addr,
        tx_data: txs,
    };

    // Parallel mining with thread cap
    if num_threads <= 1 {
        // Single-threaded path (for testing/debugging)
        return mine_single_threaded(&template, &prev_hash, miner_addr, &difficulty_target, stop, db);
    }

    // Multi-threaded mining using std::thread::scope for safe borrowing of `stop` flag
    let found = AtomicBool::new(false);
    let result: Mutex<Option<(StoredBlock, [u8; 32])>> = Mutex::new(None);
    let nonce_counter = AtomicU64::new(0);

    std::thread::scope(|s| {
        for _thread_id in 0..num_threads {
            let template = &template;
            let found = &found;
            let result = &result;
            let nonce_counter = &nonce_counter;
            let db = db.clone();

            s.spawn(move || {
                let mut engine = new_ponc_engine();
                let params = db.get_governance_params().unwrap_or_default();
                engine.pin_mut().set_rounds(params.ponc_rounds as usize);
                engine.pin_mut().initialize_scratchpad(&prev_hash, miner_addr);

                loop {
                    if found.load(Ordering::Relaxed) || stop.load(Ordering::Relaxed) {
                        return;
                    }

                    let nonce = nonce_counter.fetch_add(1, Ordering::Relaxed);
                    
                    // Update global nonce counter for hashrate tracking
                    if let Some(gc) = global_nonce_counter {
                        gc.fetch_add(1, Ordering::Relaxed);
                    }

                    let mut prefix = Vec::with_capacity(140);
                    prefix.extend_from_slice(&template.version);
                    prefix.extend_from_slice(&template.previous_hash);
                    prefix.extend_from_slice(&template.merkle_root);
                    prefix.extend_from_slice(&template.timestamp);
                    prefix.extend_from_slice(&template.difficulty_target);
                    prefix.extend_from_slice(&template.block_height);
                    prefix.extend_from_slice(&template.miner_address);

                    let mut out = [0u8; 32];
                    if engine.compute_and_verify(&prefix, nonce, &difficulty_target, &mut out) {
                        found.store(true, Ordering::SeqCst);

                        let mut block = template.clone();
                        block.nonce = nonce.to_le_bytes();
                        let hash = block_hash(&block);
                        
                        if let Ok(mut res) = result.lock() {
                            *res = Some((block, hash));
                        }
                        return;
                    }

                    if nonce % 10_000 == 0 {
                        std::thread::yield_now();
                    }
                }
            });
        }
    });

    result.into_inner().ok()?
}

// Single-threaded mining (original implementation, kept for compatibility)
fn mine_single_threaded(
    template: &StoredBlock,
    prev_hash: &[u8; 32],
    miner_addr: &[u8; 32],
    difficulty_target: &[u8; 32],
    stop: &AtomicBool,
    db: &ChainDB,
) -> Option<(StoredBlock, [u8; 32])> {
    let mut engine = new_ponc_engine();
    
    // Get current PONC rounds from governance params
    let params = db.get_governance_params().unwrap_or_default();
    engine.pin_mut().set_rounds(params.ponc_rounds as usize);
    
    engine.pin_mut().initialize_scratchpad(prev_hash, miner_addr);

    let mut nonce: u64 = 0;
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }

        let mut block = template.clone();
        block.nonce = nonce.to_le_bytes();
        
        let mut prefix = Vec::with_capacity(140);
        prefix.extend_from_slice(&block.version);
        prefix.extend_from_slice(&block.previous_hash);
        prefix.extend_from_slice(&block.merkle_root);
        prefix.extend_from_slice(&block.timestamp);
        prefix.extend_from_slice(&block.difficulty_target);
        prefix.extend_from_slice(&block.block_height);
        prefix.extend_from_slice(&block.miner_address);

        let mut out = [0u8; 32];
        if engine.compute_and_verify(&prefix, nonce, difficulty_target, &mut out) {
            let hash = block_hash(&block);
            return Some((block, hash));
        }

        nonce = nonce.wrapping_add(1);
        if nonce.is_multiple_of(10_000) {
            std::thread::yield_now();
        }
    }
}

pub fn generate_blocks(
    db: &ChainDB,
    mempool: &mut Mempool,
    miner_addr: &[u8; 32],
    count: u32,
    referrer: Option<[u8; 32]>,
) -> Vec<[u8; 32]> {
    let stop = AtomicBool::new(false);
    let mut hashes = Vec::new();
    for _ in 0..count {
        let txs = mempool.get_top_transactions(MAX_TXS);
        if let Some((block, hash)) = mine_block(db, txs, miner_addr, None, &stop, referrer)
            && apply_block(db, &block).is_ok()
        {
            hashes.push(hash);
        }
    }
    hashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::genesis::create_genesis_block;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> ChainDB {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let p = PathBuf::from(format!("/tmp/knot_mine_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&p);
        ChainDB::open(&p).unwrap()
    }

    #[test]
    fn test_mine_block1() {
        let db = tmp();
        let mut pool = Mempool::new();
        apply_block(&db, &create_genesis_block()).unwrap();

        let stop = std::sync::atomic::AtomicBool::new(false);
        let miner = [0x55u8; 32];
        let txs = pool.get_top_transactions(MAX_TXS);
        let (block, _) = mine_block(&db, txs, &miner, None, &stop, None).unwrap();
        assert_eq!(u32::from_le_bytes(block.block_height), 1);

        apply_block(&db, &block).expect("failed to apply mined block");
        assert!(db.get_account(&miner).unwrap().balance > 0);
    }
}
