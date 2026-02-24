// Trees:
//   blocks          — hash[32]      → StoredBlock bytes
//   heights         — height[4] BE  → hash[32]
//   accounts        — addr[32]      → AccountState bytes
//   meta            — "tip"         → hash[32]
//   referral_index  — code[8]       → addr[32]

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::crypto::keys::ADDRESS_BYTES;

const TREE_BLOCKS: &str = "blocks";
const TREE_HEIGHTS: &str = "heights";
const TREE_ACCOUNTS: &str = "accounts";
const TREE_META: &str = "meta";
const TREE_REFERRAL_INDEX: &str = "referral_index";
const TREE_GOV_TALLIES: &str = "gov_tallies";
const TREE_GOV_VOTES: &str = "gov_votes";
pub const KEY_TIP: &[u8] = b"tip";
pub const KEY_GOV_PARAMS: &[u8] = b"gov_params";

#[derive(Debug, Clone)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub referrer: Option<[u8; ADDRESS_BYTES]>,
    pub last_mined_height: u64,
    pub total_referred_miners: u64,
    pub total_referral_bonus_earned: u64,
    pub governance_weight: u64,
    pub total_blocks_mined: u64,
}

impl AccountState {
    pub fn empty() -> Self {
        AccountState {
            balance: 0,
            nonce: 0,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 0,
            total_blocks_mined: 0,
        }
    }

    // Byte layout (append-only, older readers tolerate missing tail fields):
    //   [0..8]   balance (LE u64)
    //   [8..16]  nonce (LE u64)
    //   [16]     has_referrer flag (0|1)
    //   [17..49] referrer addr (only if flag == 1)
    //   [...+8]  last_mined_height (LE u64)
    //   [...+8]  total_referred_miners (LE u64)      — v2+
    //   [...+8]  total_referral_bonus_earned (LE u64) — v2+
    //   [...+8]  governance_weight (LE u64)           — v3+
    //   [...+8]  total_blocks_mined (LE u64)          — v4+
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(80);
        b.extend_from_slice(&self.balance.to_le_bytes());
        b.extend_from_slice(&self.nonce.to_le_bytes());

        match self.referrer {
            Some(r) => {
                b.push(1);
                b.extend_from_slice(&r);
            }
            None => {
                b.push(0);
            }
        }

        b.extend_from_slice(&self.last_mined_height.to_le_bytes());
        b.extend_from_slice(&self.total_referred_miners.to_le_bytes());
        b.extend_from_slice(&self.total_referral_bonus_earned.to_le_bytes());
        b.extend_from_slice(&self.governance_weight.to_le_bytes());
        b.extend_from_slice(&self.total_blocks_mined.to_le_bytes());
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<Self, &'static str> {
        // Minimum: balance(8) + nonce(8) + flag(1) = 17 bytes
        if d.len() < 17 {
            return Err("account record too short");
        }

        let balance = u64::from_le_bytes(d[0..8].try_into().unwrap());
        let nonce = u64::from_le_bytes(d[8..16].try_into().unwrap());

        let mut off = 17usize;
        let referrer = if d[16] == 1 {
            if d.len() < 49 {
                return Err("truncated referrer");
            }
            let mut r = [0u8; 32];
            r.copy_from_slice(&d[17..49]);
            off = 49;
            Some(r)
        } else {
            None
        };

        let read_u64 = |o: usize, data: &[u8]| -> u64 {
            if data.len() >= o + 8 {
                u64::from_le_bytes(data[o..o + 8].try_into().unwrap())
            } else {
                0
            }
        };

        let last_mined_height = read_u64(off, d);
        off += 8;
        let total_referred_miners = read_u64(off, d);
        off += 8;
        let total_referral_bonus_earned = read_u64(off, d);
        off += 8;
        let governance_weight = read_u64(off, d);
        off += 8;
        let total_blocks_mined = read_u64(off, d);

        Ok(AccountState {
            balance,
            nonce,
            referrer,
            last_mined_height,
            total_referred_miners,
            total_referral_bonus_earned,
            governance_weight,
            total_blocks_mined,
        })
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredBlock {
    pub version: [u8; 4],
    pub previous_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub timestamp: [u8; 4],
    pub difficulty_target: [u8; 32],
    pub nonce: [u8; 8],
    pub block_height: [u8; 4],
    pub miner_address: [u8; 32],
    pub tx_data: Vec<StoredTransaction>,
}

impl StoredBlock {
    pub fn header_bytes(&self) -> [u8; 148] {
        let mut buf = [0u8; 148];
        buf[0..4].copy_from_slice(&self.version);
        buf[4..36].copy_from_slice(&self.previous_hash);
        buf[36..68].copy_from_slice(&self.merkle_root);
        buf[68..72].copy_from_slice(&self.timestamp);
        buf[72..104].copy_from_slice(&self.difficulty_target);
        buf[104..112].copy_from_slice(&self.nonce);
        buf[112..116].copy_from_slice(&self.block_height);
        buf[116..148].copy_from_slice(&self.miner_address);
        buf
    }

    pub fn header_prefix(&self) -> [u8; 140] {
        let mut buf = [0u8; 140];
        buf[0..4].copy_from_slice(&self.version);
        buf[4..36].copy_from_slice(&self.previous_hash);
        buf[36..68].copy_from_slice(&self.merkle_root);
        buf[68..72].copy_from_slice(&self.timestamp);
        buf[72..104].copy_from_slice(&self.difficulty_target);
        buf[104..108].copy_from_slice(&self.block_height);
        buf[108..140].copy_from_slice(&self.miner_address);
        buf
    }


    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.version);
        b.extend_from_slice(&self.previous_hash);
        b.extend_from_slice(&self.merkle_root);
        b.extend_from_slice(&self.timestamp);
        b.extend_from_slice(&self.difficulty_target);
        b.extend_from_slice(&self.nonce);
        b.extend_from_slice(&self.block_height);
        b.extend_from_slice(&self.miner_address);
        b.extend_from_slice(&(self.tx_data.len() as u32).to_le_bytes());
        for tx in &self.tx_data {
            b.extend_from_slice(&tx.to_bytes());
        }
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<Self, &'static str> {
        if d.len() < 148 {
            return Err("block header too short");
        }
        let mut off = 0usize;

        macro_rules! read {
            ($n:expr) => {{
                let slice = &d[off..off + $n];
                off += $n;
                slice
            }};
        }

        let mut version = [0u8; 4];
        version.copy_from_slice(read!(4));
        let mut previous_hash = [0u8; 32];
        previous_hash.copy_from_slice(read!(32));
        let mut merkle_root = [0u8; 32];
        merkle_root.copy_from_slice(read!(32));
        let mut timestamp = [0u8; 4];
        timestamp.copy_from_slice(read!(4));
        let mut difficulty_target = [0u8; 32];
        difficulty_target.copy_from_slice(read!(32));
        let mut nonce = [0u8; 8];
        nonce.copy_from_slice(read!(8));
        let mut block_height = [0u8; 4];
        block_height.copy_from_slice(read!(4));
        let mut miner_address = [0u8; 32];
        miner_address.copy_from_slice(read!(32));

        let mut tx_data = Vec::new();
        if d.len() >= off + 4 {
            let tx_count = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            for _ in 0..tx_count {
                let (tx, n) = StoredTransaction::from_bytes(&d[off..])?;
                tx_data.push(tx);
                off += n;
            }
        }

        Ok(StoredBlock {
            version,
            previous_hash,
            merkle_root,
            timestamp: timestamp[0..4].try_into().unwrap(),
            difficulty_target,
            nonce,
            block_height: block_height[0..4].try_into().unwrap(),
            miner_address,
            tx_data,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredTransaction {
    pub version: u8,
    pub sender_address: [u8; 32],
    pub sender_pubkey: Vec<u8>,
    pub recipient_address: [u8; 32],
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub timestamp: u64,
    pub referrer_address: Option<[u8; 32]>,
    pub governance_data: Option<[u8; 32]>,
    pub signature: Vec<u8>,
}

impl StoredTransaction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.push(self.version);
        b.extend_from_slice(&self.sender_address);
        b.extend_from_slice(&(self.sender_pubkey.len() as u32).to_le_bytes());
        b.extend_from_slice(&self.sender_pubkey);
        b.extend_from_slice(&self.recipient_address);
        b.extend_from_slice(&self.amount.to_le_bytes());
        b.extend_from_slice(&self.fee.to_le_bytes());
        b.extend_from_slice(&self.nonce.to_le_bytes());
        b.extend_from_slice(&self.timestamp.to_le_bytes());
        match self.referrer_address {
            Some(r) => {
                b.push(1);
                b.extend_from_slice(&r);
            }
            None => {
                b.push(0);
            }
        }
        match self.governance_data {
            Some(g) => {
                b.push(1);
                b.extend_from_slice(&g);
            }
            None => {
                b.push(0);
            }
        }
        b.extend_from_slice(&(self.signature.len() as u32).to_le_bytes());
        b.extend_from_slice(&self.signature);
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<(Self, usize), &'static str> {
        let mut off = 0usize;

        if d.is_empty() {
            return Err("tx too short: missing version");
        }
        let version = d[off];
        off += 1;

        if d.len() < off + 32 {
            return Err("tx: missing sender_address");
        }
        let mut sender_address = [0u8; 32];
        sender_address.copy_from_slice(&d[off..off + 32]);
        off += 32;

        if d.len() < off + 4 {
            return Err("tx: missing pubkey len");
        }
        let pk_len = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        if d.len() < off + pk_len {
            return Err("tx: missing pubkey data");
        }
        let sender_pubkey = d[off..off + pk_len].to_vec();
        off += pk_len;

        if d.len() < off + 32 {
            return Err("tx: missing recipient");
        }
        let mut recipient_address = [0u8; 32];
        recipient_address.copy_from_slice(&d[off..off + 32]);
        off += 32;

        if d.len() < off + 32 {
            return Err("tx: missing core scalar fields (amount, fee, nonce, timestamp)");
        }
        let amount = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let fee = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let nonce = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let timestamp = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;

        let referrer_address = if d.len() > off {
            let flag = d[off];
            off += 1;
            if flag == 1 {
                if d.len() < off + 32 {
                    return Err("tx: truncated referrer address");
                }
                let mut r = [0u8; 32];
                r.copy_from_slice(&d[off..off + 32]);
                off += 32;
                Some(r)
            } else {
                None
            }
        } else {
            None
        };

        let governance_data = if d.len() > off {
            let flag = d[off];
            off += 1;
            if flag == 1 {
                if d.len() < off + 32 {
                    return Err("tx: truncated governance data");
                }
                let mut g = [0u8; 32];
                g.copy_from_slice(&d[off..off + 32]);
                off += 32;
                Some(g)
            } else {
                None
            }
        } else {
            None
        };

        let signature = if d.len() >= off + 4 {
            let sig_len = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if d.len() < off + sig_len {
                return Err("tx: truncated signature");
            }
            let s = d[off..off + sig_len].to_vec();
            off += sig_len;
            s
        } else {
            vec![]
        };

        Ok((
            StoredTransaction {
                version,
                sender_address,
                sender_pubkey,
                recipient_address,
                amount,
                fee,
                nonce,
                timestamp,
                referrer_address,
                governance_data,
                signature,
            },
            off,
        ))
    }
}

#[derive(Clone)]
pub struct ChainDB {
    _db: sled::Db,
    blocks: sled::Tree,
    heights: sled::Tree,
    pub accounts: sled::Tree,
    meta: sled::Tree,
    referral_index: sled::Tree,
    gov_tallies: sled::Tree,
    gov_votes: sled::Tree,
}

impl ChainDB {
    pub fn open(path: &Path) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        let blocks = db.open_tree(TREE_BLOCKS)?;
        let heights = db.open_tree(TREE_HEIGHTS)?;
        let accounts = db.open_tree(TREE_ACCOUNTS)?;
        let meta = db.open_tree(TREE_META)?;
        let referral_index = db.open_tree(TREE_REFERRAL_INDEX)?;
        let gov_tallies = db.open_tree(TREE_GOV_TALLIES)?;
        let gov_votes = db.open_tree(TREE_GOV_VOTES)?;
        Ok(ChainDB {
            _db: db,
            blocks,
            heights,
            accounts,
            meta,
            referral_index,
            gov_tallies,
            gov_votes,
        })
    }

    pub fn apply_block_data_batch(&self, b_blocks: sled::Batch, b_heights: sled::Batch) -> Result<(), sled::Error> {
        self.blocks.apply_batch(b_blocks)?;
        self.heights.apply_batch(b_heights)?;
        Ok(())
    }

    pub fn apply_referral_batch(&self, batch: sled::Batch) -> Result<(), sled::Error> {
        self.referral_index.apply_batch(batch)?;
        Ok(())
    }

    pub fn apply_account_batch(&self, batch: sled::Batch) -> Result<(), sled::Error> {
        self.accounts.apply_batch(batch)?;
        Ok(())
    }

    pub fn apply_metadata_batch(&self, batch: sled::Batch) -> Result<(), sled::Error> {
        self.meta.apply_batch(batch)?;
        Ok(())
    }

    pub fn apply_governance_batch(&self, t_batch: sled::Batch, v_batch: sled::Batch) -> Result<(), sled::Error> {
        self.gov_tallies.apply_batch(t_batch)?;
        self.gov_votes.apply_batch(v_batch)?;
        Ok(())
    }

    pub fn store_block(&self, hash: &[u8; 32], block: &StoredBlock) -> Result<(), sled::Error> {
        self.blocks.insert(hash, block.to_bytes())?;
        self.heights.insert(block.block_height, hash.as_ref())?;
        Ok(())
    }

    pub fn store_block_batch(
        &self,
        hash: &[u8; 32],
        block: &StoredBlock,
        b_blocks: &mut sled::Batch,
        b_heights: &mut sled::Batch,
    ) {
        b_blocks.insert(&hash[..], block.to_bytes());
        b_heights.insert(&block.block_height[..], &hash[..]);
    }

    pub fn get_block(&self, hash: &[u8; 32]) -> Result<Option<StoredBlock>, sled::Error> {
        match self.blocks.get(hash)? {
            Some(d) => Ok(Some(StoredBlock::from_bytes(&d).expect("corrupt block"))),
            None => Ok(None),
        }
    }

    pub fn get_block_hash_by_height(&self, height: u32) -> Result<Option<[u8; 32]>, sled::Error> {
        match self.heights.get(height.to_le_bytes())? {
            Some(d) => {
                let mut h = [0u8; 32];
                h.copy_from_slice(&d);
                Ok(Some(h))
            }
            None => Ok(None),
        }
    }

    pub fn get_account(&self, addr: &[u8; 32]) -> Result<AccountState, sled::Error> {
        match self.accounts.get(addr)? {
            Some(d) => Ok(AccountState::from_bytes(&d).unwrap_or_else(|_| AccountState::empty())),
            None => Ok(AccountState::empty()),
        }
    }

    pub fn put_account(&self, addr: &[u8; 32], state: &AccountState) -> Result<(), sled::Error> {
        self.accounts.insert(addr, state.to_bytes())?;
        self.index_referral_code(addr)?;
        Ok(())
    }

    // Referral index: first 8 bytes of SHA3-256(addr) → addr.
    // Collision probability is negligible for any realistic network size.
    fn index_referral_code(&self, addr: &[u8; 32]) -> Result<(), sled::Error> {
        let hash = crate::crypto::hash::hash_sha3_256(addr);
        self.referral_index.insert(&hash[..8], addr.as_ref())?;
        Ok(())
    }

    pub fn get_address_by_referral_code(
        &self,
        code: &[u8; 8],
    ) -> Result<Option<[u8; 32]>, sled::Error> {
        match self.referral_index.get(code)? {
            Some(d) => {
                let mut a = [0u8; 32];
                a.copy_from_slice(&d);
                Ok(Some(a))
            }
            None => Ok(None),
        }
    }

    pub fn set_tip(&self, hash: &[u8; 32]) -> Result<(), sled::Error> {
        self.meta.insert(KEY_TIP, hash.as_ref())?;
        Ok(())
    }

    pub fn get_tip(&self) -> Result<Option<[u8; 32]>, sled::Error> {
        match self.meta.get(KEY_TIP)? {
            Some(d) => {
                let mut h = [0u8; 32];
                h.copy_from_slice(&d);
                Ok(Some(h))
            }
            None => Ok(None),
        }
    }

    pub fn get_chain_height(&self) -> Result<u32, sled::Error> {
        match self.get_tip()? {
            Some(h) => match self.get_block(&h)? {
                Some(b) => Ok(u32::from_le_bytes(b.block_height)),
                None => Ok(0),
            },
            None => Ok(0),
        }
    }

    pub fn flush(&self) -> Result<(), sled::Error> {
        self._db.flush().map(|_| ())
    }

    pub fn get_governance_tally(&self, proposal_hash: &[u8; 32]) -> Result<u64, sled::Error> {
        match self.gov_tallies.get(proposal_hash)? {
            Some(d) => Ok(u64::from_le_bytes(d[..8].try_into().unwrap())),
            None => Ok(0),
        }
    }

    pub fn add_governance_vote(
        &self,
        proposal_hash: &[u8; 32],
        voter: &[u8; 32],
        weight: u64,
    ) -> Result<(), sled::Error> {
        let mut vote_key = [0u8; 64];
        vote_key[..32].copy_from_slice(proposal_hash);
        vote_key[32..].copy_from_slice(voter);

        // Check if already voted
        if self.gov_votes.contains_key(vote_key)? {
            // Already voted. In production, we might allow weight updates.
            // For now, we ignore secondary votes from the same address for the same proposal.
            return Ok(());
        }

        let current = self.get_governance_tally(proposal_hash)?;
        let new = current.saturating_add(weight);
        self.gov_tallies.insert(proposal_hash, &new.to_le_bytes())?;
        self.gov_votes.insert(vote_key, &[1])?;
        Ok(())
    }

    pub fn get_governance_vote_exists(
        &self,
        proposal_hash: &[u8; 32],
        voter: &[u8; 32],
    ) -> Result<bool, sled::Error> {
        let mut vote_key = [0u8; 64];
        vote_key[..32].copy_from_slice(proposal_hash);
        vote_key[32..].copy_from_slice(voter);
        self.gov_votes.contains_key(vote_key)
    }

    pub fn get_governance_params(&self) -> Result<crate::consensus::state::GovernanceParams, sled::Error> {
        match self.meta.get(KEY_GOV_PARAMS)? {
            Some(d) => {
                if d.len() >= 16 {
                    let cap_bps = u64::from_le_bytes(d[0..8].try_into().unwrap());
                    let ponc_rounds = u64::from_le_bytes(d[8..16].try_into().unwrap());
                    Ok(crate::consensus::state::GovernanceParams { cap_bps, ponc_rounds })
                } else {
                    Ok(crate::consensus::state::GovernanceParams::default())
                }
            }
            None => Ok(crate::consensus::state::GovernanceParams::default()),
        }
    }

    pub fn set_governance_params(&self, params: &crate::consensus::state::GovernanceParams) -> Result<(), sled::Error> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&params.cap_bps.to_le_bytes());
        buf.extend_from_slice(&params.ponc_rounds.to_le_bytes());
        self.meta.insert(KEY_GOV_PARAMS, buf)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> ChainDB {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let p = PathBuf::from(format!("/tmp/knot_db_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&p);
        ChainDB::open(&p).unwrap()
    }

    #[test]
    fn test_account_roundtrip() {
        let db = tmp();
        let addr = [0xABu8; 32];
        let s = AccountState {
            balance: 500_000_000,
            nonce: 3,
            referrer: Some([0xCDu8; 32]),
            last_mined_height: 42,
            total_referred_miners: 5,
            total_referral_bonus_earned: 25_000_000,
            governance_weight: 600,
            total_blocks_mined: 10,
        };
        db.put_account(&addr, &s).unwrap();
        let got = db.get_account(&addr).unwrap();
        assert_eq!(got.balance, 500_000_000);
        assert_eq!(got.nonce, 3);
        assert_eq!(got.last_mined_height, 42);
        assert_eq!(got.total_referred_miners, 5);
        assert_eq!(got.governance_weight, 600);
    }

    #[test]
    fn test_missing_account_is_empty() {
        let db = tmp();
        let s = db.get_account(&[0xFFu8; 32]).unwrap();
        assert_eq!(s.balance, 0);
        assert_eq!(s.nonce, 0);
    }

    #[test]
    fn test_block_store_and_tip() {
        let db = tmp();
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 100u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: [1u8; 32],
            tx_data: vec![],
        };
        let hash = [0x42u8; 32];
        db.store_block(&hash, &block).unwrap();
        db.set_tip(&hash).unwrap();
        let got = db.get_block(&hash).unwrap().unwrap();
        assert_eq!(got.miner_address, [1u8; 32]);
        assert_eq!(db.get_tip().unwrap().unwrap(), hash);
        assert_eq!(db.get_chain_height().unwrap(), 0);
    }

    #[test]
    fn test_governance_tallying() {
        let db = tmp();
        let prop = [0x55u8; 32];
        let voter1 = [0x11u8; 32];
        let voter2 = [0x22u8; 32];

        // 1. Initial tally should be 0
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 0);

        // 2. Add first vote
        db.add_governance_vote(&prop, &voter1, 500).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 500);

        // 3. Duplicate vote from voter1 should be ignored
        db.add_governance_vote(&prop, &voter1, 500).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 500);

        // 4. Vote from voter2 adds up
        db.add_governance_vote(&prop, &voter2, 300).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 800);
    }

    #[test]
    fn test_governance_params_storage() {
        let db = tmp();
        
        // Default params
        let default_params = db.get_governance_params().unwrap();
        assert_eq!(default_params.cap_bps, crate::consensus::chain::GOVERNANCE_CAP_DEFAULT_BPS);
        assert_eq!(default_params.ponc_rounds, crate::consensus::chain::PONC_ROUNDS_DEFAULT);
        
        // Set custom params
        let custom_params = crate::consensus::state::GovernanceParams {
            cap_bps: 750,
            ponc_rounds: 1024,
        };
        db.set_governance_params(&custom_params).unwrap();
        
        // Retrieve and verify
        let retrieved = db.get_governance_params().unwrap();
        assert_eq!(retrieved.cap_bps, 750);
        assert_eq!(retrieved.ponc_rounds, 1024);
    }

    #[test]
    fn test_referral_code_indexing() {
        let db = tmp();
        let addr = [0xAAu8; 32];
        let state = AccountState::empty();
        
        db.put_account(&addr, &state).unwrap();
        
        // Get referral code
        let code = crate::crypto::hash::hash_sha3_256(&addr);
        let mut code_bytes = [0u8; 8];
        code_bytes.copy_from_slice(&code[..8]);
        
        // Lookup by code
        let found = db.get_address_by_referral_code(&code_bytes).unwrap();
        assert_eq!(found, Some(addr));
    }

    #[test]
    fn test_block_height_lookup() {
        let db = tmp();
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 100u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 5u32.to_le_bytes(),
            miner_address: [1u8; 32],
            tx_data: vec![],
        };
        let hash = [0x42u8; 32];
        
        db.store_block(&hash, &block).unwrap();
        
        let found_hash = db.get_block_hash_by_height(5).unwrap();
        assert_eq!(found_hash, Some(hash));
        
        let not_found = db.get_block_hash_by_height(10).unwrap();
        assert_eq!(not_found, None);
    }

    #[test]
    fn test_account_state_serialization() {
        let state = AccountState {
            balance: 123_456_789,
            nonce: 42,
            referrer: Some([0xABu8; 32]),
            last_mined_height: 1000,
            total_referred_miners: 5,
            total_referral_bonus_earned: 50_000_000,
            governance_weight: 300,
            total_blocks_mined: 15,
        };
        
        let bytes = state.to_bytes();
        let decoded = AccountState::from_bytes(&bytes).unwrap();
        
        assert_eq!(decoded.balance, state.balance);
        assert_eq!(decoded.nonce, state.nonce);
        assert_eq!(decoded.referrer, state.referrer);
        assert_eq!(decoded.last_mined_height, state.last_mined_height);
        assert_eq!(decoded.total_referred_miners, state.total_referred_miners);
        assert_eq!(decoded.total_referral_bonus_earned, state.total_referral_bonus_earned);
        assert_eq!(decoded.governance_weight, state.governance_weight);
        assert_eq!(decoded.total_blocks_mined, state.total_blocks_mined);
    }

    #[test]
    fn test_account_state_no_referrer() {
        let state = AccountState {
            balance: 100,
            nonce: 1,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 100,
            total_blocks_mined: 0,
        };
        
        let bytes = state.to_bytes();
        let decoded = AccountState::from_bytes(&bytes).unwrap();
        
        assert_eq!(decoded.referrer, None);
    }

    #[test]
    fn test_multiple_proposals() {
        let db = tmp();
        let prop1 = [0x11u8; 32];
        let prop2 = [0x22u8; 32];
        let voter = [0xAAu8; 32];
        
        db.add_governance_vote(&prop1, &voter, 100).unwrap();
        db.add_governance_vote(&prop2, &voter, 200).unwrap();
        
        assert_eq!(db.get_governance_tally(&prop1).unwrap(), 100);
        assert_eq!(db.get_governance_tally(&prop2).unwrap(), 200);
    }

    #[test]
    fn test_chain_height_empty() {
        let db = tmp();
        assert_eq!(db.get_chain_height().unwrap(), 0);
    }
}
