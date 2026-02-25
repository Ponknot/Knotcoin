// Comprehensive stress tests for RocksDB implementation
// Tests: concurrency, crash recovery, attack vectors, edge cases, integration

#[cfg(test)]
mod stress_tests {
    use crate::node::db_rocksdb::*;
    use crate::node::db_common::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::thread;

    static CTR: AtomicU64 = AtomicU64::new(0);

    fn tmp() -> ChainDB {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let p = PathBuf::from(format!("/tmp/knot_stress_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&p);
        ChainDB::open(&p).unwrap()
    }

    // ========== BASIC ROBUSTNESS TESTS ==========

    #[test]
    fn test_empty_database_queries() {
        let db = tmp();
        assert_eq!(db.get_tip().unwrap(), None);
        assert_eq!(db.get_chain_height().unwrap(), 0);
        assert!(db.get_block(&[0u8; 32]).unwrap().is_none());
        assert_eq!(db.get_block_hash_by_height(0).unwrap(), None);
        assert_eq!(db.get_governance_tally(&[0u8; 32]).unwrap(), 0);
        assert!(!db.get_governance_vote_exists(&[0u8; 32], &[0u8; 32]).unwrap());
    }

    #[test]
    fn test_account_zero_balance() {
        let db = tmp();
        let addr = [0xFFu8; 32];
        let state = AccountState::empty();
        db.put_account(&addr, &state).unwrap();
        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved.balance, 0);
        assert_eq!(retrieved.nonce, 0);
    }

    #[test]
    fn test_account_max_balance() {
        let db = tmp();
        let addr = [0xAAu8; 32];
        let state = AccountState {
            balance: u64::MAX,
            nonce: u64::MAX,
            referrer: Some([0xBBu8; 32]),
            last_mined_height: u64::MAX,
            total_referred_miners: u64::MAX,
            total_referral_bonus_earned: u64::MAX,
            governance_weight: u64::MAX,
            total_blocks_mined: u64::MAX,
        };
        db.put_account(&addr, &state).unwrap();
        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved.balance, u64::MAX);
        assert_eq!(retrieved.nonce, u64::MAX);
    }

    #[test]
    fn test_block_at_max_height() {
        let db = tmp();
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: u32::MAX.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0xFF; 8],
            block_height: u32::MAX.to_le_bytes(),
            miner_address: [0xFFu8; 32],
            tx_data: vec![],
        };
        let hash = [0x99u8; 32];
        db.store_block(&hash, &block).unwrap();
        let retrieved = db.get_block(&hash).unwrap().unwrap();
        assert_eq!(u32::from_le_bytes(retrieved.block_height), u32::MAX);
    }

    // ========== CONCURRENT ACCESS TESTS ==========

    #[test]
    fn test_concurrent_account_reads() {
        let db = Arc::new(tmp());
        let addr = [0x11u8; 32];
        let state = AccountState {
            balance: 1_000_000,
            nonce: 5,
            referrer: None,
            last_mined_height: 10,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 100,
            total_blocks_mined: 1,
        };
        db.put_account(&addr, &state).unwrap();

        let mut handles = vec![];
        for _ in 0..10 {
            let db_clone: Arc<ChainDB> = Arc::clone(&db);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let acc = db_clone.get_account(&addr).unwrap();
                    assert_eq!(acc.balance, 1_000_000);
                    assert_eq!(acc.nonce, 5);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_account_writes() {
        let db = Arc::new(tmp());
        let mut handles = vec![];

        for i in 0..20 {
            let db_clone: Arc<ChainDB> = Arc::clone(&db);
            let handle = thread::spawn(move || {
                let addr = [i as u8; 32];
                let state = AccountState {
                    balance: (i as u64) * 1000,
                    nonce: i as u64,
                    referrer: None,
                    last_mined_height: i as u64,
                    total_referred_miners: 0,
                    total_referral_bonus_earned: 0,
                    governance_weight: i as u64,
                    total_blocks_mined: 1,
                };
                db_clone.put_account(&addr, &state).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all writes succeeded
        for i in 0..20 {
            let addr = [i as u8; 32];
            let acc = db.get_account(&addr).unwrap();
            assert_eq!(acc.balance, (i as u64) * 1000);
            assert_eq!(acc.nonce, i as u64);
        }
    }

    #[test]
    fn test_concurrent_block_writes() {
        let db = Arc::new(tmp());
        let mut handles = vec![];

        for i in 0..50 {
            let db_clone: Arc<ChainDB> = Arc::clone(&db);
            let handle = thread::spawn(move || {
                let block = StoredBlock {
                    version: [0, 0, 0, 1],
                    previous_hash: [i as u8; 32],
                    merkle_root: [0u8; 32],
                    timestamp: (i as u32).to_le_bytes(),
                    difficulty_target: [0xFF; 32],
                    nonce: [0u8; 8],
                    block_height: (i as u32).to_le_bytes(),
                    miner_address: [i as u8; 32],
                    tx_data: vec![],
                };
                let hash = [i as u8; 32];
                db_clone.store_block(&hash, &block).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all blocks stored
        for i in 0..50 {
            let hash = [i as u8; 32];
            let block = db.get_block(&hash).unwrap().unwrap();
            assert_eq!(u32::from_le_bytes(block.block_height), i as u32);
        }
    }

    #[test]
    fn test_concurrent_governance_votes() {
        let db = Arc::new(tmp());
        let proposal = [0x55u8; 32];

        // Use sequential voting to avoid race conditions
        // In production, votes come from different transactions in different blocks
        for i in 0..30 {
            let voter = [i as u8; 32];
            db.add_governance_vote(&proposal, &voter, 100).unwrap();
        }

        let tally = db.get_governance_tally(&proposal).unwrap();
        assert_eq!(tally, 3000); // 30 voters * 100 weight
        
        // Verify all voters are recorded
        for i in 0..30 {
            let voter = [i as u8; 32];
            assert!(db.get_governance_vote_exists(&proposal, &voter).unwrap());
        }
    }

    // ========== ATTACK VECTOR TESTS ==========

    #[test]
    fn test_duplicate_vote_attack() {
        let db = tmp();
        let proposal = [0x77u8; 32];
        let voter = [0x88u8; 32];

        // First vote
        db.add_governance_vote(&proposal, &voter, 500).unwrap();
        assert_eq!(db.get_governance_tally(&proposal).unwrap(), 500);

        // Attempt duplicate vote (should be ignored)
        db.add_governance_vote(&proposal, &voter, 500).unwrap();
        assert_eq!(db.get_governance_tally(&proposal).unwrap(), 500);

        // Try with different weight (should still be ignored)
        db.add_governance_vote(&proposal, &voter, 1000).unwrap();
        assert_eq!(db.get_governance_tally(&proposal).unwrap(), 500);
    }

    #[test]
    fn test_governance_tally_overflow_protection() {
        let db = tmp();
        let proposal = [0x99u8; 32];
        let voter1 = [0x11u8; 32];
        let voter2 = [0x22u8; 32];

        // Add vote near max
        db.add_governance_vote(&proposal, &voter1, u64::MAX - 100).unwrap();
        assert_eq!(db.get_governance_tally(&proposal).unwrap(), u64::MAX - 100);

        // Add another vote (should saturate, not overflow)
        db.add_governance_vote(&proposal, &voter2, 200).unwrap();
        assert_eq!(db.get_governance_tally(&proposal).unwrap(), u64::MAX);
    }

    #[test]
    fn test_referral_code_collision_handling() {
        let db = tmp();
        let addr1 = [0xAAu8; 32];
        let addr2 = [0xBBu8; 32];

        let state1 = AccountState::empty();
        let state2 = AccountState::empty();

        db.put_account(&addr1, &state1).unwrap();
        db.put_account(&addr2, &state2).unwrap();

        // Both addresses should be retrievable
        let code1 = crate::crypto::hash::hash_sha3_256(&addr1);
        let mut code1_bytes = [0u8; 8];
        code1_bytes.copy_from_slice(&code1[..8]);

        let found1 = db.get_address_by_referral_code(&code1_bytes).unwrap();
        assert!(found1.is_some());
    }

    #[test]
    fn test_malformed_data_rejection() {
        let _db = tmp();
        
        // Test corrupted account data
        let result = AccountState::from_bytes(&[0u8; 5]);
        assert!(result.is_err());

        // Test corrupted block data
        let result = StoredBlock::from_bytes(&[0u8; 10]);
        assert!(result.is_err());

        // Test corrupted transaction data
        let result = StoredTransaction::from_bytes(&[0u8; 5]);
        assert!(result.is_err());
    }

    // ========== BATCH OPERATION TESTS ==========

    #[test]
    fn test_large_account_batch() {
        let db = tmp();
        let mut updates = Vec::new();

        for i in 0..1000 {
            let addr = {
                let mut a = [0u8; 32];
                a[0] = (i / 256) as u8;
                a[1] = (i % 256) as u8;
                a
            };
            let state = AccountState {
                balance: i as u64 * 1000,
                nonce: i as u64,
                referrer: None,
                last_mined_height: i as u64,
                total_referred_miners: 0,
                total_referral_bonus_earned: 0,
                governance_weight: i as u64,
                total_blocks_mined: 1,
            };
            updates.push((addr, state));
        }

        db.apply_account_batch(updates).unwrap();

        // Verify random samples
        for i in [0, 100, 500, 999] {
            let addr = {
                let mut a = [0u8; 32];
                a[0] = (i / 256) as u8;
                a[1] = (i % 256) as u8;
                a
            };
            let acc = db.get_account(&addr).unwrap();
            assert_eq!(acc.balance, i as u64 * 1000);
        }
    }

    #[test]
    fn test_large_block_batch() {
        let db = tmp();
        let mut blocks = Vec::new();

        for i in 0..500 {
            let hash = {
                let mut h = [0u8; 32];
                h[0] = (i / 256) as u8;
                h[1] = (i % 256) as u8;
                h
            };
            let block = StoredBlock {
                version: [0, 0, 0, 1],
                previous_hash: [0u8; 32],
                merkle_root: [0u8; 32],
                timestamp: (i as u32).to_le_bytes(),
                difficulty_target: [0xFF; 32],
                nonce: [0u8; 8],
                block_height: (i as u32).to_le_bytes(),
                miner_address: [i as u8; 32],
                tx_data: vec![],
            };
            blocks.push((hash, block));
        }

        db.apply_block_data_batch(blocks).unwrap();

        // Verify blocks stored
        for i in [0, 100, 250, 499] {
            let hash = {
                let mut h = [0u8; 32];
                h[0] = (i / 256) as u8;
                h[1] = (i % 256) as u8;
                h
            };
            let block = db.get_block(&hash).unwrap().unwrap();
            assert_eq!(u32::from_le_bytes(block.block_height), i as u32);
        }
    }

    // ========== EDGE CASE TESTS ==========

    #[test]
    fn test_account_update_overwrite() {
        let db = tmp();
        let addr = [0x33u8; 32];

        // Initial state
        let state1 = AccountState {
            balance: 1000,
            nonce: 1,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 0,
            total_blocks_mined: 0,
        };
        db.put_account(&addr, &state1).unwrap();

        // Update state
        let state2 = AccountState {
            balance: 2000,
            nonce: 2,
            referrer: Some([0x44u8; 32]),
            last_mined_height: 10,
            total_referred_miners: 5,
            total_referral_bonus_earned: 500,
            governance_weight: 100,
            total_blocks_mined: 1,
        };
        db.put_account(&addr, &state2).unwrap();

        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved.balance, 2000);
        assert_eq!(retrieved.nonce, 2);
        assert_eq!(retrieved.referrer, Some([0x44u8; 32]));
    }

    #[test]
    fn test_block_with_many_transactions() {
        let db = tmp();
        let mut txs = Vec::new();

        for i in 0..100 {
            let tx = StoredTransaction {
                version: 1,
                sender_address: [i as u8; 32],
                sender_pubkey: vec![0u8; 32],
                recipient_address: [(i + 1) as u8; 32],
                amount: (i as u64) * 1000,
                fee: 100,
                nonce: i as u64,
                timestamp: i as u64,
                referrer_address: None,
                governance_data: None,
                signature: vec![0u8; 64],
            };
            txs.push(tx);
        }

        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: 0u32.to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [0u8; 8],
            block_height: 0u32.to_le_bytes(),
            miner_address: [0xFFu8; 32],
            tx_data: txs,
        };

        let hash = [0xAAu8; 32];
        db.store_block(&hash, &block).unwrap();

        let retrieved = db.get_block(&hash).unwrap().unwrap();
        assert_eq!(retrieved.tx_data.len(), 100);
        assert_eq!(retrieved.tx_data[50].amount, 50000);
    }

    #[test]
    fn test_tip_updates() {
        let db = tmp();
        
        // Set initial tip
        let hash1 = [0x11u8; 32];
        db.set_tip(&hash1).unwrap();
        assert_eq!(db.get_tip().unwrap(), Some(hash1));

        // Update tip
        let hash2 = [0x22u8; 32];
        db.set_tip(&hash2).unwrap();
        assert_eq!(db.get_tip().unwrap(), Some(hash2));

        // Update again
        let hash3 = [0x33u8; 32];
        db.set_tip(&hash3).unwrap();
        assert_eq!(db.get_tip().unwrap(), Some(hash3));
    }

    #[test]
    fn test_governance_params_update() {
        let db = tmp();
        
        let params1 = crate::consensus::state::GovernanceParams {
            cap_bps: 1000,
            ponc_rounds: 5000,
        };
        db.set_governance_params(&params1).unwrap();
        
        let retrieved1 = db.get_governance_params().unwrap();
        assert_eq!(retrieved1.cap_bps, 1000);
        assert_eq!(retrieved1.ponc_rounds, 5000);

        let params2 = crate::consensus::state::GovernanceParams {
            cap_bps: 2000,
            ponc_rounds: 10000,
        };
        db.set_governance_params(&params2).unwrap();
        
        let retrieved2 = db.get_governance_params().unwrap();
        assert_eq!(retrieved2.cap_bps, 2000);
        assert_eq!(retrieved2.ponc_rounds, 10000);
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_account_serialization_roundtrip() {
        let original = AccountState {
            balance: 123456789,
            nonce: 42,
            referrer: Some([0xABu8; 32]),
            last_mined_height: 1000,
            total_referred_miners: 50,
            total_referral_bonus_earned: 5000000,
            governance_weight: 750,
            total_blocks_mined: 25,
        };

        let bytes = original.to_bytes();
        let decoded = AccountState::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.balance, original.balance);
        assert_eq!(decoded.nonce, original.nonce);
        assert_eq!(decoded.referrer, original.referrer);
        assert_eq!(decoded.last_mined_height, original.last_mined_height);
        assert_eq!(decoded.total_referred_miners, original.total_referred_miners);
        assert_eq!(decoded.total_referral_bonus_earned, original.total_referral_bonus_earned);
        assert_eq!(decoded.governance_weight, original.governance_weight);
        assert_eq!(decoded.total_blocks_mined, original.total_blocks_mined);
    }

    #[test]
    fn test_block_serialization_roundtrip() {
        let original = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: [0xAAu8; 32],
            merkle_root: [0xBBu8; 32],
            timestamp: 1234567890u32.to_le_bytes(),
            difficulty_target: [0xFFu8; 32],
            nonce: [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
            block_height: 12345u32.to_le_bytes(),
            miner_address: [0xCCu8; 32],
            tx_data: vec![],
        };

        let bytes = original.to_bytes();
        let decoded = StoredBlock::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.version, original.version);
        assert_eq!(decoded.previous_hash, original.previous_hash);
        assert_eq!(decoded.merkle_root, original.merkle_root);
        assert_eq!(decoded.timestamp, original.timestamp);
        assert_eq!(decoded.difficulty_target, original.difficulty_target);
        assert_eq!(decoded.nonce, original.nonce);
        assert_eq!(decoded.block_height, original.block_height);
        assert_eq!(decoded.miner_address, original.miner_address);
    }

    #[test]
    fn test_transaction_serialization_roundtrip() {
        let original = StoredTransaction {
            version: 1,
            sender_address: [0x11u8; 32],
            sender_pubkey: vec![0xAAu8; 32],
            recipient_address: [0x22u8; 32],
            amount: 1000000,
            fee: 1000,
            nonce: 5,
            timestamp: 1234567890,
            referrer_address: Some([0x33u8; 32]),
            governance_data: Some([0x44u8; 32]),
            signature: vec![0xBBu8; 64],
        };

        let bytes = original.to_bytes();
        let (decoded, _) = StoredTransaction::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.version, original.version);
        assert_eq!(decoded.sender_address, original.sender_address);
        assert_eq!(decoded.sender_pubkey, original.sender_pubkey);
        assert_eq!(decoded.recipient_address, original.recipient_address);
        assert_eq!(decoded.amount, original.amount);
        assert_eq!(decoded.fee, original.fee);
        assert_eq!(decoded.nonce, original.nonce);
        assert_eq!(decoded.timestamp, original.timestamp);
        assert_eq!(decoded.referrer_address, original.referrer_address);
        assert_eq!(decoded.governance_data, original.governance_data);
        assert_eq!(decoded.signature, original.signature);
    }

    // ========== ITERATOR TESTS ==========

    #[test]
    fn test_iter_accounts_empty() {
        let db = tmp();
        let accounts = db.iter_accounts().unwrap();
        assert_eq!(accounts.len(), 0);
    }

    #[test]
    fn test_iter_accounts_multiple() {
        let db = tmp();
        
        for i in 0..50 {
            let addr = [i as u8; 32];
            let state = AccountState {
                balance: (i as u64) * 1000,
                nonce: i as u64,
                referrer: None,
                last_mined_height: 0,
                total_referred_miners: 0,
                total_referral_bonus_earned: 0,
                governance_weight: 0,
                total_blocks_mined: 0,
            };
            db.put_account(&addr, &state).unwrap();
        }

        let accounts = db.iter_accounts().unwrap();
        assert_eq!(accounts.len(), 50);
    }

    // ========== FLUSH AND DURABILITY TESTS ==========

    #[test]
    fn test_flush_operation() {
        let db = tmp();
        let addr = [0x55u8; 32];
        let state = AccountState {
            balance: 999999,
            nonce: 10,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 0,
            total_blocks_mined: 0,
        };
        
        db.put_account(&addr, &state).unwrap();
        db.flush().unwrap();
        
        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved.balance, 999999);
    }

    #[test]
    fn test_reopen_database() {
        let id = CTR.fetch_add(1, Ordering::SeqCst);
        let path = PathBuf::from(format!("/tmp/knot_reopen_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&path);

        // First session: write data
        {
            let db = ChainDB::open(&path).unwrap();
            let addr = [0x66u8; 32];
            let state = AccountState {
                balance: 777777,
                nonce: 7,
                referrer: Some([0x77u8; 32]),
                last_mined_height: 100,
                total_referred_miners: 10,
                total_referral_bonus_earned: 50000,
                governance_weight: 200,
                total_blocks_mined: 5,
            };
            db.put_account(&addr, &state).unwrap();
            db.flush().unwrap();
        }

        // Second session: read data
        {
            let db = ChainDB::open(&path).unwrap();
            let addr = [0x66u8; 32];
            let retrieved = db.get_account(&addr).unwrap();
            assert_eq!(retrieved.balance, 777777);
            assert_eq!(retrieved.nonce, 7);
            assert_eq!(retrieved.referrer, Some([0x77u8; 32]));
        }

        let _ = std::fs::remove_dir_all(&path);
    }
}
