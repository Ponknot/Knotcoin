use crate::consensus::chain::{
    calculate_block_reward, calculate_governance_weight, calculate_referral_bonus,
    GOVERNANCE_CAP_DEFAULT_BPS, PONC_ROUNDS_DEFAULT,
};
use crate::crypto::hash::hash_sha3_256;
use crate::crypto::ponc::ffi::bridge::new_ponc_engine;
use crate::node::db::{ChainDB, StoredBlock};
use crate::primitives::transaction::Transaction;

#[derive(Debug, Clone)]
pub struct GovernanceParams {
    pub cap_bps: u64,
    pub ponc_rounds: u64,
}

impl Default for GovernanceParams {
    fn default() -> Self {
        Self {
            cap_bps: GOVERNANCE_CAP_DEFAULT_BPS,
            ponc_rounds: PONC_ROUNDS_DEFAULT,
        }
    }
}

#[derive(Debug)]
pub enum StateError {
    InsufficientBalance,
    InvalidNonce { expected: u64, got: u64 },
    DuplicateReferrer,
    SelfReferral,
    InvalidCoinbase,
    MathOverflow,
    Storage(sled::Error),
    InvalidPoW,
    InvalidTransaction(&'static str),
    BlockInPast,
    BlockTooFarInFuture,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::InsufficientBalance => write!(f, "insufficient balance"),
            StateError::InvalidNonce { expected, got } => {
                write!(f, "bad nonce: want {expected}, got {got}")
            }
            StateError::DuplicateReferrer => write!(f, "referrer already set"),
            StateError::SelfReferral => write!(f, "cannot refer yourself"),
            StateError::InvalidCoinbase => write!(f, "invalid coinbase"),
            StateError::MathOverflow => write!(f, "mathematical overflow"),
            StateError::Storage(e) => write!(f, "storage: {e}"),
            StateError::InvalidPoW => write!(f, "invalid proof-of-work hash"),
            StateError::InvalidTransaction(e) => {
                write!(f, "transaction validation failed: {e}")
            }
            StateError::BlockInPast => write!(f, "block timestamp is before median-time-past"),
            StateError::BlockTooFarInFuture => write!(f, "block timestamp is too far in future"),
        }
    }
}

impl std::error::Error for StateError {}

// MANUALLY JUSTIFIED UNSAFE BLOCKS
// StateError wraps sled::Error, which is thread-safe in practice, but
// manual Send/Sync are required for seamless async propagation.
unsafe impl Send for StateError {}
unsafe impl Sync for StateError {}

impl From<sled::Error> for StateError {
    fn from(e: sled::Error) -> Self {
        StateError::Storage(e)
    }
}

/// Verify block PoW without state access (stateless, can be parallelized)
/// This is consensus-safe to call in parallel across multiple blocks
pub fn verify_block_pow(block: &StoredBlock, db: &ChainDB) -> Result<(), StateError> {
    let height = u32::from_le_bytes(block.block_height) as u64;
    
    // Skip PoW verification for genesis block
    if height == 0 {
        return Ok(());
    }
    
    let mut engine = new_ponc_engine();
    
    // Get current PONC rounds from governance params
    let params = db.get_governance_params().map_err(|_| StateError::Storage(
        sled::Error::Unsupported("failed to read governance params".to_string())
    ))?;
    engine.pin_mut().set_rounds(params.ponc_rounds as usize);
    
    engine
        .pin_mut()
        .initialize_scratchpad(&block.previous_hash, &block.miner_address);

    let mut prefix = Vec::with_capacity(140);
    prefix.extend_from_slice(&block.version);
    prefix.extend_from_slice(&block.previous_hash);
    prefix.extend_from_slice(&block.merkle_root);
    prefix.extend_from_slice(&block.timestamp);
    prefix.extend_from_slice(&block.difficulty_target);
    prefix.extend_from_slice(&block.block_height);
    prefix.extend_from_slice(&block.miner_address);

    let nonce = u64::from_le_bytes(block.nonce);
    let mut out = [0u8; 32];
    if !engine.compute_and_verify(&prefix, nonce, &block.difficulty_target, &mut out) {
        return Err(StateError::InvalidPoW);
    }
    
    Ok(())
}

pub fn apply_block(db: &ChainDB, block: &StoredBlock) -> Result<(), StateError> {
    let height = u32::from_le_bytes(block.block_height) as u64;
    let block_time = u32::from_le_bytes(block.timestamp);

    // 0. Verify Timestamp (MTP + Future Limit)
    if height > 0 {
        let mut times = Vec::new();
        // Look back up to 11 blocks for MTP
        for i in 1..=11 {
            if height >= i
                && let Ok(Some(h)) = db.get_block_hash_by_height((height - i) as u32)
                && let Ok(Some(b)) = db.get_block(&h)
            {
                times.push(u32::from_le_bytes(b.timestamp));
            }
        }
        if !times.is_empty() {
            times.sort();
            let mtp = times[times.len() / 2];
            if block_time <= mtp {
                return Err(StateError::BlockInPast);
            }
        }
    }

    // Future limit: no more than 2 hours (7200s) ahead of now
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    if block_time > now + 7200 {
        return Err(StateError::BlockTooFarInFuture);
    }

    // 1. Verify PoW (Strict Mainnet Requirement)
    verify_block_pow(block, db)?;

    // 2. Calculate Rewards
    let base_reward = calculate_block_reward(height);

    let mut account_updates: std::collections::HashMap<[u8; 32], crate::node::db::AccountState> = std::collections::HashMap::new();
    let mut tally_updates: std::collections::HashMap<[u8; 32], u64> = std::collections::HashMap::new();
    let mut vote_keys = Vec::new();

    let get_account_local = |addr: &[u8; 32], updates: &std::collections::HashMap<[u8; 32], crate::node::db::AccountState>, db: &ChainDB| -> crate::node::db::AccountState {
        updates.get(addr).cloned().unwrap_or_else(|| db.get_account(addr).unwrap_or_default())
    };

    // Credit base reward to miner first
    let mut miner_acc = get_account_local(&block.miner_address, &account_updates, db);
    miner_acc.balance = miner_acc.balance.checked_add(base_reward).ok_or(StateError::MathOverflow)?;
    miner_acc.last_mined_height = height;
    miner_acc.total_blocks_mined = miner_acc.total_blocks_mined.saturating_add(1);
    miner_acc.governance_weight = calculate_governance_weight(miner_acc.total_blocks_mined);

    // Referral bonus
    if let Some(ref_addr) = miner_acc.referrer {
        let mut referrer = get_account_local(&ref_addr, &account_updates, db);
        let bonus = calculate_referral_bonus(base_reward, referrer.last_mined_height, height);
        if bonus > 0 {
            referrer.balance = referrer.balance.checked_add(bonus).ok_or(StateError::MathOverflow)?;
            referrer.total_referral_bonus_earned = referrer.total_referral_bonus_earned.checked_add(bonus).ok_or(StateError::MathOverflow)?;
            referrer.governance_weight = calculate_governance_weight(referrer.total_referred_miners);
            account_updates.insert(ref_addr, referrer);
        }
    }
    account_updates.insert(block.miner_address, miner_acc);

    let mut fees = 0u64;
    let mut seen_txids = std::collections::HashSet::new();
    
    for tx in &block.tx_data {
        let domain_tx = Transaction::try_from(tx).map_err(StateError::InvalidTransaction)?;
        if !domain_tx.is_structurally_valid() {
            return Err(StateError::InvalidTransaction("structural or signature failure"));
        }

        // Check for duplicate TXIDs within this block
        let txid = domain_tx.txid();
        if !seen_txids.insert(txid) {
            return Err(StateError::InvalidTransaction("duplicate transaction in block"));
        }

        fees = fees.checked_add(tx.fee).ok_or(StateError::MathOverflow)?;

        let mut sender = get_account_local(&tx.sender_address, &account_updates, db);
        let debit = tx.amount.checked_add(tx.fee).ok_or(StateError::MathOverflow)?;

        if sender.balance < debit {
            return Err(StateError::InsufficientBalance);
        }
        let want = sender.nonce + 1;
        if tx.nonce != want {
            return Err(StateError::InvalidNonce { expected: want, got: tx.nonce });
        }

        sender.balance = sender.balance.checked_sub(debit).ok_or(StateError::MathOverflow)?;
        sender.nonce = tx.nonce;

        // Governance signaling (Move this BEFORE account_updates.insert)
        if let Some(prop_hash) = tx.governance_data {
            let mut vote_key = [0u8; 64];
            vote_key[..32].copy_from_slice(&prop_hash);
            vote_key[32..].copy_from_slice(&tx.sender_address);
            
            if !db.get_governance_vote_exists(&prop_hash, &tx.sender_address)? {
                let current_tally = tally_updates.get(&prop_hash).cloned().unwrap_or_else(|| db.get_governance_tally(&prop_hash).unwrap_or(0));
                let new_tally = current_tally.saturating_add(sender.governance_weight);
                tally_updates.insert(prop_hash, new_tally);
                vote_keys.push(vote_key);
            }
        }

        // Referral registration
        if tx.nonce == 1 && let Some(ref_addr) = tx.referrer_address {
            if sender.referrer.is_some() {
                return Err(StateError::DuplicateReferrer);
            }
            if ref_addr == tx.sender_address {
                return Err(StateError::SelfReferral);
            }
            sender.referrer = Some(ref_addr);
            let mut upstream = get_account_local(&ref_addr, &account_updates, db);
            upstream.total_referred_miners = upstream.total_referred_miners.checked_add(1).ok_or(StateError::MathOverflow)?;
            upstream.governance_weight = calculate_governance_weight(upstream.total_referred_miners);
            account_updates.insert(ref_addr, upstream);
        }

        account_updates.insert(tx.sender_address, sender);

        let mut recipient = get_account_local(&tx.recipient_address, &account_updates, db);
        recipient.balance = recipient.balance.checked_add(tx.amount).ok_or(StateError::MathOverflow)?;
        account_updates.insert(tx.recipient_address, recipient);
    }

    // 5. Credit accumulated fees to miner
    let mut miner_with_fees = account_updates.get(&block.miner_address).cloned().unwrap();
    miner_with_fees.balance = miner_with_fees.balance.checked_add(fees).ok_or(StateError::MathOverflow)?;
    account_updates.insert(block.miner_address, miner_with_fees);

    // 5. Build and Apply Batches
    let mut b_accounts = sled::Batch::default();
    let mut b_ref = sled::Batch::default();
    let mut b_tallies = sled::Batch::default();
    let mut b_votes = sled::Batch::default();
    let mut b_meta = sled::Batch::default();
    let mut b_blocks = sled::Batch::default();
    let mut b_heights = sled::Batch::default();

    for (addr, state) in account_updates {
        b_accounts.insert(&addr[..], state.to_bytes());
        let h = crate::crypto::hash::hash_sha3_256(&addr);
        b_ref.insert(&h[..8], &addr[..]);
    }

    for (prop, tally) in tally_updates {
        b_tallies.insert(&prop[..], &tally.to_le_bytes());
    }

    for vkey in vote_keys {
        b_votes.insert(&vkey[..], &[1]);
    }

    let hash = block_hash(block);
    b_blocks.insert(&hash[..], block.to_bytes());
    b_heights.insert(&block.block_height[..], &hash[..]);
    b_meta.insert(crate::node::db::KEY_TIP, &hash[..]);

    // Apply in strict order: Data -> State -> Tip
    db.apply_block_data_batch(b_blocks, b_heights)?;
    db.apply_account_batch(b_accounts)?;
    db.apply_referral_batch(b_ref)?;
    db.apply_governance_batch(b_tallies, b_votes)?;
    db.apply_metadata_batch(b_meta)?;

    Ok(())
}

pub fn block_hash(block: &StoredBlock) -> [u8; 32] {
    hash_sha3_256(&block.header_bytes())
}

// Keep the old name as an alias so callers in knotcoind / miner don't break.
pub use block_hash as compute_stored_block_hash;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::db::StoredBlock;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> ChainDB {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let p = PathBuf::from(format!("/tmp/knot_state_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&p);
        ChainDB::open(&p).unwrap()
    }

    #[test]
    fn test_apply_genesis() {
        let db = tmp();
        let miner = [0x01u8; 32];
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: miner,
            tx_data: vec![],
        };
        apply_block(&db, &block).unwrap();
        let s = db.get_account(&miner).unwrap();
        assert_eq!(s.balance, 10_000_000); // block 0 reward = 0.1 KOT (10M Knots)
        assert_eq!(s.last_mined_height, 0);
    }

    #[test]
    fn test_governance_params_default() {
        let params = GovernanceParams::default();
        assert_eq!(params.cap_bps, GOVERNANCE_CAP_DEFAULT_BPS);
        assert_eq!(params.ponc_rounds, PONC_ROUNDS_DEFAULT);
    }

    #[test]
    fn test_multiple_blocks() {
        let db = tmp();
        let miner = [0x02u8; 32];
        
        // Apply genesis
        let genesis = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: miner,
            tx_data: vec![],
        };
        apply_block(&db, &genesis).unwrap();
        
        // Apply block 1
        let block1 = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: block_hash(&genesis),
            merkle_root: [0u8; 32],
            timestamp: 60u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [1u8; 8],
            block_height: 1u32.to_le_bytes(),
            miner_address: miner,
            tx_data: vec![],
        };
        apply_block(&db, &block1).unwrap();
        
        let s = db.get_account(&miner).unwrap();
        assert_eq!(s.total_blocks_mined, 2);
        assert_eq!(s.last_mined_height, 1);
    }

    #[test]
    fn test_block_hash_deterministic() {
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: [0x01u8; 32],
            tx_data: vec![],
        };
        
        let hash1 = block_hash(&block);
        let hash2 = block_hash(&block);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_blocks_different_hashes() {
        let block1 = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: [0x01u8; 32],
            tx_data: vec![],
        };
        
        let block2 = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [1u8; 8], // Different nonce
            block_height: 0u32.to_le_bytes(),
            miner_address: [0x01u8; 32],
            tx_data: vec![],
        };
        
        assert_ne!(block_hash(&block1), block_hash(&block2));
    }
}
