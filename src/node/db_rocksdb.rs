// RocksDB-based blockchain database implementation
// Replaces sled with production-grade embedded database
// 
// Design Principles:
// 1. Durability - WAL enabled, sync on block commits
// 2. Performance - Tuned for blockchain workloads
// 3. Atomicity - Batch operations for multi-tree updates
// 4. Crash Recovery - Automatic via RocksDB WAL
//
// Column Families (equivalent to sled Trees):
// - "blocks"          : hash[32] → StoredBlock bytes
// - "heights"         : height[4] LE → hash[32]
// - "accounts"        : addr[32] → AccountState bytes
// - "meta"            : string keys → various values
// - "referral_index"  : code[8] → addr[32]
// - "gov_tallies"     : proposal[32] → tally[8]
// - "gov_votes"       : proposal[32]+voter[32] → flag[1]

use rocksdb::{DB, Options, WriteBatch, ColumnFamilyDescriptor, SliceTransform};
use std::path::Path;
use std::sync::Arc;

// Column family names (must match sled tree names for compatibility)
const CF_BLOCKS: &str = "blocks";
const CF_HEIGHTS: &str = "heights";
const CF_ACCOUNTS: &str = "accounts";
const CF_META: &str = "meta";
const CF_REFERRAL_INDEX: &str = "referral_index";
const CF_GOV_TALLIES: &str = "gov_tallies";
const CF_GOV_VOTES: &str = "gov_votes";

// Metadata keys
pub const KEY_TIP: &[u8] = b"tip";
pub const KEY_GOV_PARAMS: &[u8] = b"gov_params";

// Re-export types from db_common
pub use super::db_common::{AccountState, StoredBlock, StoredTransaction};

/// Custom error type for database operations
#[derive(Debug)]
pub enum DbError {
    RocksDb(rocksdb::Error),
    Corruption(&'static str),
    NotFound,
}

impl From<rocksdb::Error> for DbError {
    fn from(e: rocksdb::Error) -> Self {
        DbError::RocksDb(e)
    }
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::RocksDb(e) => write!(f, "RocksDB error: {}", e),
            DbError::Corruption(msg) => write!(f, "Data corruption: {}", msg),
            DbError::NotFound => write!(f, "Key not found"),
        }
    }
}

impl std::error::Error for DbError {}

/// Main database handle with column families
#[derive(Clone)]
pub struct ChainDB {
    pub db: Arc<DB>,
}

impl ChainDB {
    /// Open or create database with optimized settings for blockchain workloads
    /// 
    /// Performance Tuning Rationale:
    /// - write_buffer_size: 64MB - Balance between memory and flush frequency
    ///   Larger = fewer flushes but more memory. 64MB good for 60-second blocks.
    /// - max_write_buffer_number: 3 - Allow 3 memtables before blocking writes
    ///   Prevents write stalls during compaction.
    /// - target_file_size_base: 64MB - SST file size target
    ///   Matches write buffer for efficient compaction.
    /// - compression: LZ4 - Fast compression with good ratio
    ///   Blockchain data compresses well (lots of zeros in hashes).
    /// - prefix_extractor: 8 bytes - Optimize for referral code lookups
    ///   Referral codes are 8-byte prefixes of SHA3 hashes.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        // Base options for all column families
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        // Write buffer settings - tuned for blockchain
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64 MB
        opts.set_max_write_buffer_number(3);
        opts.set_min_write_buffer_number_to_merge(1);
        
        // SST file settings
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64 MB
        opts.set_max_bytes_for_level_base(256 * 1024 * 1024); // 256 MB
        
        // Compression - LZ4 for speed
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        
        // Block cache - 256 MB for hot data
        let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_block_size(16 * 1024); // 16 KB blocks
        opts.set_block_based_table_factory(&block_opts);
        
        // WAL settings - critical for crash recovery
        opts.set_wal_bytes_per_sync(1024 * 1024); // Sync WAL every 1 MB
        opts.set_max_total_wal_size(128 * 1024 * 1024); // 128 MB max WAL
        
        // Compaction settings
        opts.set_level_compaction_dynamic_level_bytes(true);
        opts.set_max_background_jobs(4); // Parallel compaction
        
        // Column family descriptors
        let cf_blocks = ColumnFamilyDescriptor::new(CF_BLOCKS, opts.clone());
        let cf_heights = ColumnFamilyDescriptor::new(CF_HEIGHTS, opts.clone());
        let cf_accounts = ColumnFamilyDescriptor::new(CF_ACCOUNTS, opts.clone());
        let cf_meta = ColumnFamilyDescriptor::new(CF_META, opts.clone());
        
        // Referral index with prefix extractor for efficient lookups
        let mut ref_opts = opts.clone();
        ref_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(8));
        let cf_referral = ColumnFamilyDescriptor::new(CF_REFERRAL_INDEX, ref_opts);
        
        let cf_gov_tallies = ColumnFamilyDescriptor::new(CF_GOV_TALLIES, opts.clone());
        let cf_gov_votes = ColumnFamilyDescriptor::new(CF_GOV_VOTES, opts.clone());
        
        let cfs = vec![
            cf_blocks,
            cf_heights,
            cf_accounts,
            cf_meta,
            cf_referral,
            cf_gov_tallies,
            cf_gov_votes,
        ];
        
        // Open database with all column families
        let db = DB::open_cf_descriptors(&opts, path, cfs)?;
        
        Ok(ChainDB {
            db: Arc::new(db),
        })
    }
    
    /// Get column family handle (internal helper)
    fn cf(&self, name: &str) -> Result<&rocksdb::ColumnFamily, DbError> {
        self.db.cf_handle(name)
            .ok_or_else(|| DbError::Corruption("column family not found"))
    }
    
    // ========== BLOCK OPERATIONS ==========
    
    /// Store a block atomically with its height index
    /// 
    /// Atomicity Reasoning:
    /// - Both block and height index must be written together
    /// - If crash happens mid-write, neither should be visible
    /// - WriteBatch ensures atomicity via WAL
    pub fn store_block(&self, hash: &[u8; 32], block: &StoredBlock) -> Result<(), DbError> {
        let mut batch = WriteBatch::default();
        
        let cf_blocks = self.cf(CF_BLOCKS)?;
        let cf_heights = self.cf(CF_HEIGHTS)?;
        
        batch.put_cf(cf_blocks, hash, block.to_bytes());
        batch.put_cf(cf_heights, &block.block_height, hash);
        
        // Write atomically with sync for durability
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(true); // Force fsync for block commits
        
        self.db.write_opt(batch, &write_opts)?;
        Ok(())
    }
    
    /// Add block to batch (for bulk operations)
    pub fn store_block_batch(
        &self,
        hash: &[u8; 32],
        block: &StoredBlock,
        batch: &mut WriteBatch,
    ) -> Result<(), DbError> {
        let cf_blocks = self.cf(CF_BLOCKS)?;
        let cf_heights = self.cf(CF_HEIGHTS)?;
        
        batch.put_cf(cf_blocks, hash, block.to_bytes());
        batch.put_cf(cf_heights, &block.block_height, hash);
        Ok(())
    }
    
    /// Retrieve block by hash
    pub fn get_block(&self, hash: &[u8; 32]) -> Result<Option<StoredBlock>, DbError> {
        let cf = self.cf(CF_BLOCKS)?;
        
        match self.db.get_cf(cf, hash)? {
            Some(data) => {
                let block = StoredBlock::from_bytes(&data)
                    .map_err(|e| DbError::Corruption(e))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }
    
    /// Get block hash by height
    pub fn get_block_hash_by_height(&self, height: u32) -> Result<Option<[u8; 32]>, DbError> {
        let cf = self.cf(CF_HEIGHTS)?;
        
        match self.db.get_cf(cf, height.to_le_bytes())? {
            Some(data) => {
                if data.len() != 32 {
                    return Err(DbError::Corruption("invalid hash length"));
                }
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data);
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }
    
    // ========== ACCOUNT OPERATIONS ==========
    
    /// Get account state (returns empty if not found)
    /// 
    /// Design Decision: Return empty instead of Option
    /// Reasoning: Simplifies caller code, matches blockchain semantics
    /// (non-existent account = zero balance account)
    pub fn get_account(&self, addr: &[u8; 32]) -> Result<AccountState, DbError> {
        let cf = self.cf(CF_ACCOUNTS)?;
        
        match self.db.get_cf(cf, addr)? {
            Some(data) => {
                AccountState::from_bytes(&data)
                    .map_err(|e| DbError::Corruption(e))
            }
            None => Ok(AccountState::empty()),
        }
    }
    
    /// Store account state and update referral index
    pub fn put_account(&self, addr: &[u8; 32], state: &AccountState) -> Result<(), DbError> {
        let mut batch = WriteBatch::default();
        
        let cf_accounts = self.cf(CF_ACCOUNTS)?;
        let cf_referral = self.cf(CF_REFERRAL_INDEX)?;
        
        batch.put_cf(cf_accounts, addr, state.to_bytes());
        
        // Update referral index
        let hash = crate::crypto::hash::hash_sha3_256(addr);
        batch.put_cf(cf_referral, &hash[..8], addr);
        
        self.db.write(batch)?;
        Ok(())
    }
    
    /// Batch account updates (for block processing)
    pub fn apply_account_batch(&self, updates: Vec<([u8; 32], AccountState)>) -> Result<(), DbError> {
        let mut batch = WriteBatch::default();
        
        let cf_accounts = self.cf(CF_ACCOUNTS)?;
        let cf_referral = self.cf(CF_REFERRAL_INDEX)?;
        
        for (addr, state) in updates {
            batch.put_cf(cf_accounts, &addr, state.to_bytes());
            
            // Update referral index
            let hash = crate::crypto::hash::hash_sha3_256(&addr);
            batch.put_cf(cf_referral, &hash[..8], &addr);
        }
        
        // Sync for durability
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(true);
        
        self.db.write_opt(batch, &write_opts)?;
        Ok(())
    }
    
    // ========== REFERRAL OPERATIONS ==========
    
    /// Lookup address by referral code (first 8 bytes of SHA3-256(addr))
    /// 
    /// Collision Probability Analysis:
    /// - 8 bytes = 64 bits
    /// - Birthday paradox: ~50% collision at 2^32 addresses (4 billion)
    /// - Knotcoin unlikely to reach 4 billion addresses
    /// - If collision occurs, first-come-first-served (acceptable)
    pub fn get_address_by_referral_code(
        &self,
        code: &[u8; 8],
    ) -> Result<Option<[u8; 32]>, DbError> {
        let cf = self.cf(CF_REFERRAL_INDEX)?;
        
        match self.db.get_cf(cf, code)? {
            Some(data) => {
                if data.len() != 32 {
                    return Err(DbError::Corruption("invalid address length"));
                }
                let mut addr = [0u8; 32];
                addr.copy_from_slice(&data);
                Ok(Some(addr))
            }
            None => Ok(None),
        }
    }
    
    // ========== METADATA OPERATIONS ==========
    
    /// Set chain tip (most recent block hash)
    pub fn set_tip(&self, hash: &[u8; 32]) -> Result<(), DbError> {
        let cf = self.cf(CF_META)?;
        
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(true); // Critical metadata, must sync
        
        self.db.put_cf_opt(cf, KEY_TIP, hash, &write_opts)?;
        Ok(())
    }
    
    /// Get chain tip
    pub fn get_tip(&self) -> Result<Option<[u8; 32]>, DbError> {
        let cf = self.cf(CF_META)?;
        
        match self.db.get_cf(cf, KEY_TIP)? {
            Some(data) => {
                if data.len() != 32 {
                    return Err(DbError::Corruption("invalid tip hash length"));
                }
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data);
                Ok(Some(hash))
            }
            None => Ok(None),
        }
    }
    
    /// Get current chain height
    pub fn get_chain_height(&self) -> Result<u32, DbError> {
        match self.get_tip()? {
            Some(hash) => match self.get_block(&hash)? {
                Some(block) => Ok(u32::from_le_bytes(block.block_height)),
                None => Ok(0),
            },
            None => Ok(0),
        }
    }
    
    // ========== GOVERNANCE OPERATIONS ==========
    
    /// Get vote tally for a proposal
    pub fn get_governance_tally(&self, proposal_hash: &[u8; 32]) -> Result<u64, DbError> {
        let cf = self.cf(CF_GOV_TALLIES)?;
        
        match self.db.get_cf(cf, proposal_hash)? {
            Some(data) => {
                if data.len() != 8 {
                    return Err(DbError::Corruption("invalid tally length"));
                }
                Ok(u64::from_le_bytes(data[..8].try_into().unwrap()))
            }
            None => Ok(0),
        }
    }
    
    /// Add a governance vote (with duplicate prevention)
    /// 
    /// Atomicity Reasoning:
    /// - Vote record and tally update must be atomic
    /// - If crash happens, either both succeed or both fail
    /// - Prevents double-counting votes
    pub fn add_governance_vote(
        &self,
        proposal_hash: &[u8; 32],
        voter: &[u8; 32],
        weight: u64,
    ) -> Result<(), DbError> {
        let cf_tallies = self.cf(CF_GOV_TALLIES)?;
        let cf_votes = self.cf(CF_GOV_VOTES)?;
        
        // Create vote key: proposal_hash + voter
        let mut vote_key = [0u8; 64];
        vote_key[..32].copy_from_slice(proposal_hash);
        vote_key[32..].copy_from_slice(voter);
        
        // Check if already voted
        if self.db.get_cf(cf_votes, &vote_key)?.is_some() {
            // Already voted, ignore (idempotent)
            return Ok(());
        }
        
        // Get current tally
        let current = self.get_governance_tally(proposal_hash)?;
        let new_tally = current.saturating_add(weight);
        
        // Atomic update
        let mut batch = WriteBatch::default();
        batch.put_cf(cf_tallies, proposal_hash, &new_tally.to_le_bytes());
        batch.put_cf(cf_votes, &vote_key, &[1u8]);
        
        self.db.write(batch)?;
        Ok(())
    }
    
    /// Check if address has voted on proposal
    pub fn get_governance_vote_exists(
        &self,
        proposal_hash: &[u8; 32],
        voter: &[u8; 32],
    ) -> Result<bool, DbError> {
        let cf = self.cf(CF_GOV_VOTES)?;
        
        let mut vote_key = [0u8; 64];
        vote_key[..32].copy_from_slice(proposal_hash);
        vote_key[32..].copy_from_slice(voter);
        
        Ok(self.db.get_cf(cf, &vote_key)?.is_some())
    }
    
    /// Get governance parameters
    pub fn get_governance_params(&self) -> Result<crate::consensus::state::GovernanceParams, DbError> {
        let cf = self.cf(CF_META)?;
        
        match self.db.get_cf(cf, KEY_GOV_PARAMS)? {
            Some(data) => {
                if data.len() >= 16 {
                    let cap_bps = u64::from_le_bytes(data[0..8].try_into().unwrap());
                    let ponc_rounds = u64::from_le_bytes(data[8..16].try_into().unwrap());
                    Ok(crate::consensus::state::GovernanceParams { cap_bps, ponc_rounds })
                } else {
                    Ok(crate::consensus::state::GovernanceParams::default())
                }
            }
            None => Ok(crate::consensus::state::GovernanceParams::default()),
        }
    }
    
    /// Set governance parameters
    pub fn set_governance_params(
        &self,
        params: &crate::consensus::state::GovernanceParams,
    ) -> Result<(), DbError> {
        let cf = self.cf(CF_META)?;
        
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&params.cap_bps.to_le_bytes());
        buf.extend_from_slice(&params.ponc_rounds.to_le_bytes());
        
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(true); // Critical metadata
        
        self.db.put_cf_opt(cf, KEY_GOV_PARAMS, buf, &write_opts)?;
        Ok(())
    }
    
    // ========== BATCH OPERATIONS ==========
    
    /// Apply a batch of block data updates atomically
    pub fn apply_block_data_batch(
        &self,
        blocks: Vec<([u8; 32], StoredBlock)>,
    ) -> Result<(), DbError> {
        let mut batch = WriteBatch::default();
        
        for (hash, block) in blocks {
            self.store_block_batch(&hash, &block, &mut batch)?;
        }
        
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(true);
        
        self.db.write_opt(batch, &write_opts)?;
        Ok(())
    }
    
    /// Flush all pending writes to disk
    /// 
    /// Note: RocksDB WAL provides durability, so explicit flush
    /// is only needed for performance tuning, not correctness.
    pub fn flush(&self) -> Result<(), DbError> {
        // Flush all column families
        let cfs = vec![
            CF_BLOCKS,
            CF_HEIGHTS,
            CF_ACCOUNTS,
            CF_META,
            CF_REFERRAL_INDEX,
            CF_GOV_TALLIES,
            CF_GOV_VOTES,
        ];
        
        for cf_name in cfs {
            if let Some(cf) = self.db.cf_handle(cf_name) {
                self.db.flush_cf(cf)?;
            }
        }
        
        Ok(())
    }
    
    /// Iterate over all accounts (for RPC queries)
    /// Returns iterator of (address, AccountState) pairs
    /// 
    /// Note: This creates a snapshot and iterates over it.
    /// For large databases, consider pagination in the caller.
    pub fn iter_accounts(&self) -> Result<Vec<([u8; 32], AccountState)>, DbError> {
        let cf = self.cf(CF_ACCOUNTS)?;
        let mut results = Vec::new();
        
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        for item in iter {
            let (key, value) = item?;
            
            if key.len() != 32 {
                continue; // Skip malformed keys
            }
            
            let mut addr = [0u8; 32];
            addr.copy_from_slice(&key);
            
            match AccountState::from_bytes(&value) {
                Ok(state) => results.push((addr, state)),
                Err(_) => continue, // Skip corrupted entries
            }
        }
        
        Ok(results)
    }
}

// Implement Send + Sync for thread safety
unsafe impl Send for ChainDB {}
unsafe impl Sync for ChainDB {}

// Include comprehensive stress tests
#[cfg(test)]
#[path = "db_rocksdb_stress_tests.rs"]
mod stress_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> ChainDB {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let p = PathBuf::from(format!("/tmp/knot_rocksdb_{}_{}", std::process::id(), id));
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

        assert_eq!(db.get_governance_tally(&prop).unwrap(), 0);

        db.add_governance_vote(&prop, &voter1, 500).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 500);

        // Duplicate vote ignored
        db.add_governance_vote(&prop, &voter1, 500).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 500);

        db.add_governance_vote(&prop, &voter2, 300).unwrap();
        assert_eq!(db.get_governance_tally(&prop).unwrap(), 800);
    }

    #[test]
    fn test_referral_code_lookup() {
        let db = tmp();
        let addr = [0xAAu8; 32];
        let state = AccountState::empty();
        
        db.put_account(&addr, &state).unwrap();
        
        let code = crate::crypto::hash::hash_sha3_256(&addr);
        let mut code_bytes = [0u8; 8];
        code_bytes.copy_from_slice(&code[..8]);
        
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
}
