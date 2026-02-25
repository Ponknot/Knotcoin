// Mining loop: assemble template → initialize PONC scratchpad → iterate nonces.
// A new engine is created per template so the scratchpad is always clean.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::consensus::chain::calculate_new_difficulty;
use crate::consensus::state::{apply_block, block_hash};
use crate::crypto::hash::hash_sha3_256;
use crate::crypto::ponc::ffi::bridge::new_ponc_engine;
use crate::net::mempool::Mempool;
use crate::node::{ChainDB, db_common::{StoredBlock, StoredTransaction}};


const MAX_TXS: usize = 6;
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
    mempool: &mut Mempool,
    miner_addr: &[u8; 32],
    miner_sk: Option<&crate::crypto::dilithium::SecretKey>,
    stop: &AtomicBool,
    referrer: Option<[u8; 32]>,
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

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    let mut txs = mempool.get_top_transactions(MAX_TXS);

    // If this miner has no referrer yet and one is provided, prepend a
    // zero-fee referral-binding transaction. This is a protocol-level
    // record, not a user transaction, so it bypasses the fee floor.
    if let Some(ref_addr) = referrer {
        let miner_state = db.get_account(miner_addr).unwrap_or_default();
        if miner_state.referrer.is_none() && miner_state.nonce == 0 {
            let mut bind = StoredTransaction {
                version: 1,
                sender_address: *miner_addr,
                sender_pubkey: vec![0; 1952], // Dilithium3 public key size
                recipient_address: *miner_addr,
                amount: 0,
                fee: 0,
                nonce: 1,
                timestamp: now as u64,
                referrer_address: Some(ref_addr),
                governance_data: None,
                signature: vec![0; crate::crypto::dilithium::DILITHIUM3_SIG_BYTES],
            };
            
            // If we have the SK, sign it properly.
            if let Some(sk) = miner_sk {
                let domain_tx = crate::primitives::transaction::Transaction::try_from(&bind).ok();
                if let Some(dtx) = domain_tx {
                    let sig = crate::crypto::dilithium::sign(&dtx.signing_hash(), sk);
                    bind.signature = sig.0.to_vec();
                }
            }
            txs.insert(0, bind);
            txs.truncate(MAX_TXS);
        }
    }

    let root = merkle_root(&txs);
    let mut template = StoredBlock {
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

    let mut engine = new_ponc_engine();
    
    // Get current PONC rounds from governance params
    let params = db.get_governance_params().unwrap_or_default();
    engine.pin_mut().set_rounds(params.ponc_rounds as usize);
    
    engine
        .pin_mut()
        .initialize_scratchpad(&prev_hash, miner_addr);

    let mut nonce: u64 = 0;
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }

        template.nonce = nonce.to_le_bytes();
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
            let hash = block_hash(&template);
            return Some((template, hash));
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
        if let Some((block, hash)) = mine_block(db, mempool, miner_addr, None, &stop, referrer)
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
        let (block, _) = mine_block(&db, &mut pool, &miner, None, &stop, None).unwrap();
        assert_eq!(u32::from_le_bytes(block.block_height), 1);

        apply_block(&db, &block).expect("failed to apply mined block");
        assert!(db.get_account(&miner).unwrap().balance > 0);
    }
}
