// Integration tests: Database ↔ Consensus ↔ Network ↔ RPC
// Verifies end-to-end data flow and component interaction

use knotcoin::consensus::state::{apply_block, block_hash, GovernanceParams};
use knotcoin::node::{ChainDB, db_common::{StoredBlock, StoredTransaction, AccountState}};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static CTR: AtomicU64 = AtomicU64::new(0);

fn tmp_db() -> ChainDB {
    let id = CTR.fetch_add(1, Ordering::SeqCst);
    let p = PathBuf::from(format!("/tmp/knot_integration_{}_{}", std::process::id(), id));
    let _ = std::fs::remove_dir_all(&p);
    ChainDB::open(&p).unwrap()
}

fn create_signed_tx(seed_val: u8, nonce: u64, amount: u64, fee: u64) -> (StoredTransaction, [u8; 32], [u8; 32]) {
    use knotcoin::crypto::dilithium;
    use knotcoin::crypto::keys;
    use knotcoin::primitives::transaction::Transaction;

    let (pk, sk) = dilithium::generate_keypair(&[seed_val; 64]);
    let sender = keys::derive_address(&pk);
    let recipient = [0xEEu8; 32];

    let mut tx = Transaction {
        version: 1,
        sender_address: sender,
        sender_pubkey: pk,
        recipient_address: recipient,
        amount,
        fee,
        nonce,
        timestamp: 1000,
        referrer_address: None,
        governance_data: None,
        signature: dilithium::Signature([0u8; 3309]),
    };

    let sig = dilithium::sign(&tx.signing_hash(), &sk);
    tx.signature = sig;

    let stored = StoredTransaction {
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

    (stored, sender, recipient)
}

// ========== DATABASE ↔ CONSENSUS INTEGRATION ==========

#[test]
fn test_genesis_block_application() {
    let db = tmp_db();
    let miner = [0x01u8; 32];
    
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
    
    // Verify database state after consensus processing
    let acc = db.get_account(&miner).unwrap();
    assert_eq!(acc.balance, 10_000_000); // Genesis reward
    assert_eq!(acc.last_mined_height, 0);
    assert_eq!(acc.total_blocks_mined, 1);
    
    let hash = block_hash(&genesis);
    let stored = db.get_block(&hash).unwrap().unwrap();
    assert_eq!(stored.miner_address, miner);
    
    assert_eq!(db.get_tip().unwrap(), Some(hash));
    assert_eq!(db.get_chain_height().unwrap(), 0);
}

#[test]
fn test_multi_block_chain_building() {
    let db = tmp_db();
    let miner = [0x02u8; 32];
    
    // Genesis
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
    
    // Block 1
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
    
    // Block 2
    let block2 = StoredBlock {
        version: [0, 0, 0, 1],
        previous_hash: block_hash(&block1),
        merkle_root: [0u8; 32],
        timestamp: 120u32.to_le_bytes(),
        difficulty_target: [0xFF; 32],
        nonce: [2u8; 8],
        block_height: 2u32.to_le_bytes(),
        miner_address: miner,
        tx_data: vec![],
    };
    apply_block(&db, &block2).unwrap();
    
    // Verify chain state
    let acc = db.get_account(&miner).unwrap();
    assert_eq!(acc.total_blocks_mined, 3);
    assert_eq!(acc.last_mined_height, 2);
    
    let hash2 = block_hash(&block2);
    assert_eq!(db.get_tip().unwrap(), Some(hash2));
    assert_eq!(db.get_chain_height().unwrap(), 2);
    
    // Verify all blocks retrievable
    assert!(db.get_block(&block_hash(&genesis)).unwrap().is_some());
    assert!(db.get_block(&block_hash(&block1)).unwrap().is_some());
    assert!(db.get_block(&block_hash(&block2)).unwrap().is_some());
}

#[test]
fn test_transaction_processing_updates_accounts() {
    let db = tmp_db();
    // Create block with transaction
    let (tx, sender, recipient) = create_signed_tx(1, 1, 100_000, 1_000);
    
    // Setup: sender has balance
    let sender_state = AccountState {
        balance: 1_000_000,
        nonce: 0,
        referrer: None,
        last_mined_height: 0,
        total_referred_miners: 0,
        total_referral_bonus_earned: 0,
        governance_weight: 0,
        total_blocks_mined: 0,
    };
    db.put_account(&sender, &sender_state).unwrap();
    
    let block = StoredBlock {
        version: [0, 0, 0, 1],
        previous_hash: [0u8; 32],
        merkle_root: [0u8; 32],
        timestamp: 1000u32.to_le_bytes(),
        difficulty_target: [0xFF; 32],
        nonce: [0u8; 8],
        block_height: 0u32.to_le_bytes(),
        miner_address: [0x33u8; 32],
        tx_data: vec![tx],
    };
    
    apply_block(&db, &block).unwrap();
    
    // Verify sender balance decreased
    let sender_after = db.get_account(&sender).unwrap();
    assert_eq!(sender_after.balance, 1_000_000 - 100_000 - 1_000);
    assert_eq!(sender_after.nonce, 1);
    
    // Verify recipient balance increased
    let recipient_after = db.get_account(&recipient).unwrap();
    assert_eq!(recipient_after.balance, 100_000);
}

#[test]
fn test_referral_system_integration() {
    let db = tmp_db();
    let referrer = [0xAAu8; 32];
    let referee = [0xBBu8; 32];
    
    // Referrer mines genesis
    let genesis = StoredBlock {
        version: [0, 0, 0, 1],
        previous_hash: [0u8; 32],
        merkle_root: [0u8; 32],
        timestamp: 0u32.to_le_bytes(),
        difficulty_target: [0xFF; 32],
        nonce: [0u8; 8],
        block_height: 0u32.to_le_bytes(),
        miner_address: referrer,
        tx_data: vec![],
    };
    apply_block(&db, &genesis).unwrap();
    
    // Setup referee with referrer
    let referee_state = AccountState {
        balance: 0,
        nonce: 0,
        referrer: Some(referrer),
        last_mined_height: 0,
        total_referred_miners: 0,
        total_referral_bonus_earned: 0,
        governance_weight: 0,
        total_blocks_mined: 0,
    };
    db.put_account(&referee, &referee_state).unwrap();
    
    // Update referrer's referred count
    let mut referrer_state = db.get_account(&referrer).unwrap();
    referrer_state.total_referred_miners = 1;
    db.put_account(&referrer, &referrer_state).unwrap();
    
    // Referee mines block 1
    let block1 = StoredBlock {
        version: [0, 0, 0, 1],
        previous_hash: block_hash(&genesis),
        merkle_root: [0u8; 32],
        timestamp: 60u32.to_le_bytes(),
        difficulty_target: [0xFF; 32],
        nonce: [1u8; 8],
        block_height: 1u32.to_le_bytes(),
        miner_address: referee,
        tx_data: vec![],
    };
    apply_block(&db, &block1).unwrap();
    
    // Verify referrer got bonus
    let referrer_after = db.get_account(&referrer).unwrap();
    assert!(referrer_after.total_referral_bonus_earned > 0);
}

#[test]
fn test_governance_voting_integration() {
    let db = tmp_db();
    let proposal = [0x55u8; 32];
    let voter1 = [0x11u8; 32];
    let voter2 = [0x22u8; 32];
    
    // Setup voters with governance weight
    let state1 = AccountState {
        balance: 1_000_000,
        nonce: 0,
        referrer: None,
        last_mined_height: 0,
        total_referred_miners: 0,
        total_referral_bonus_earned: 0,
        governance_weight: 500,
        total_blocks_mined: 5,
    };
    db.put_account(&voter1, &state1).unwrap();
    
    let state2 = AccountState {
        balance: 2_000_000,
        nonce: 0,
        referrer: None,
        last_mined_height: 0,
        total_referred_miners: 0,
        total_referral_bonus_earned: 0,
        governance_weight: 300,
        total_blocks_mined: 3,
    };
    db.put_account(&voter2, &state2).unwrap();
    
    // Cast votes
    db.add_governance_vote(&proposal, &voter1, 500).unwrap();
    db.add_governance_vote(&proposal, &voter2, 300).unwrap();
    
    // Verify tally
    let tally = db.get_governance_tally(&proposal).unwrap();
    assert_eq!(tally, 800);
    
    // Verify vote records
    assert!(db.get_governance_vote_exists(&proposal, &voter1).unwrap());
    assert!(db.get_governance_vote_exists(&proposal, &voter2).unwrap());
}

#[test]
fn test_governance_params_persistence() {
    let db = tmp_db();
    
    let params = GovernanceParams {
        cap_bps: 1500,
        ponc_rounds: 8000,
        mining_threads: 4,
    };
    db.set_governance_params(&params).unwrap();
    
    let retrieved = db.get_governance_params().unwrap();
    assert_eq!(retrieved.cap_bps, 1500);
    assert_eq!(retrieved.ponc_rounds, 8000);
}

// ========== STRESS TESTS ==========

#[test]
fn test_rapid_block_sequence() {
    let db = tmp_db();
    let miner = [0x99u8; 32];
    
    let mut prev_hash = [0u8; 32];
    
    for i in 0..100 {
        let block = StoredBlock {
            version: [0, 0, 0, 1],
            previous_hash: prev_hash,
            merkle_root: [0u8; 32],
            timestamp: (i as u32 * 60).to_le_bytes(),
            difficulty_target: [0xFF; 32],
            nonce: [i as u8; 8],
            block_height: (i as u32).to_le_bytes(),
            miner_address: miner,
            tx_data: vec![],
        };
        
        apply_block(&db, &block).unwrap();
        prev_hash = block_hash(&block);
    }
    
    let acc = db.get_account(&miner).unwrap();
    assert_eq!(acc.total_blocks_mined, 100);
    assert_eq!(db.get_chain_height().unwrap(), 99);
}

#[test]
fn test_many_accounts_creation() {
    let db = tmp_db();
    
    for i in 0..500 {
        let addr = {
            let mut a = [0u8; 32];
            a[0] = (i / 256) as u8;
            a[1] = (i % 256) as u8;
            a
        };
        
        let state = AccountState {
            balance: (i as u64) * 10000,
            nonce: i as u64,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: i as u64,
            total_blocks_mined: 0,
        };
        
        db.put_account(&addr, &state).unwrap();
    }
    
    // Verify random samples
    for i in [0, 100, 250, 499] {
        let addr = {
            let mut a = [0u8; 32];
            a[0] = (i / 256) as u8;
            a[1] = (i % 256) as u8;
            a
        };
        let acc = db.get_account(&addr).unwrap();
        assert_eq!(acc.balance, (i as u64) * 10000);
    }
}

#[test]
fn test_block_with_max_transactions() {
    let db = tmp_db();
    
    // Setup sender accounts
    for i in 0..10 {
        let addr = [i as u8; 32];
        let state = AccountState {
            balance: 10_000_000,
            nonce: 0,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 0,
            total_blocks_mined: 0,
        };
        db.put_account(&addr, &state).unwrap();
    }
    
    // Create block with many transactions
    let mut txs = Vec::new();
    let mut senders = Vec::new();
    for i in 1..=10 {
        let (tx, sender, _) = create_signed_tx(i as u8, 1, 100_000, 1_000);
        // Fund sender
        let mut state = AccountState::empty();
        state.balance = 10_000_000;
        db.put_account(&sender, &state).unwrap();
        txs.push(tx);
        senders.push(sender);
    }
    
    let block = StoredBlock {
        version: [0, 0, 0, 1],
        previous_hash: [0u8; 32],
        merkle_root: [0u8; 32],
        timestamp: 1000u32.to_le_bytes(),
        difficulty_target: [0xFF; 32],
        nonce: [0u8; 8],
        block_height: 0u32.to_le_bytes(),
        miner_address: [0xFFu8; 32],
        tx_data: txs,
    };
    
    apply_block(&db, &block).unwrap();
    
    // Verify all transactions processed
    for sender in senders {
        let acc = db.get_account(&sender).unwrap();
        assert_eq!(acc.balance, 10_000_000 - 100_000 - 1_000);
        assert_eq!(acc.nonce, 1);
    }
}

// ========== DATA INTEGRITY TESTS ==========

#[test]
fn test_block_hash_consistency() {
    let block = StoredBlock {
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
    
    let hash1 = block_hash(&block);
    let hash2 = block_hash(&block);
    assert_eq!(hash1, hash2);
}

#[test]
fn test_account_state_persistence() {
    let id = CTR.fetch_add(1, Ordering::SeqCst);
    let path = PathBuf::from(format!("/tmp/knot_persist_{}_{}", std::process::id(), id));
    let _ = std::fs::remove_dir_all(&path);
    
    let addr = [0x77u8; 32];
    let original = AccountState {
        balance: 5_000_000,
        nonce: 25,
        referrer: Some([0x88u8; 32]),
        last_mined_height: 500,
        total_referred_miners: 10,
        total_referral_bonus_earned: 250_000,
        governance_weight: 400,
        total_blocks_mined: 8,
    };
    
    // Write and close
    {
        let db = ChainDB::open(&path).unwrap();
        db.put_account(&addr, &original).unwrap();
        db.flush().unwrap();
    }
    
    // Reopen and verify
    {
        let db = ChainDB::open(&path).unwrap();
        let retrieved = db.get_account(&addr).unwrap();
        assert_eq!(retrieved.balance, original.balance);
        assert_eq!(retrieved.nonce, original.nonce);
        assert_eq!(retrieved.referrer, original.referrer);
        assert_eq!(retrieved.last_mined_height, original.last_mined_height);
        assert_eq!(retrieved.total_referred_miners, original.total_referred_miners);
        assert_eq!(retrieved.total_referral_bonus_earned, original.total_referral_bonus_earned);
        assert_eq!(retrieved.governance_weight, original.governance_weight);
        assert_eq!(retrieved.total_blocks_mined, original.total_blocks_mined);
    }
    
    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn test_tip_persistence_across_restarts() {
    let id = CTR.fetch_add(1, Ordering::SeqCst);
    let path = PathBuf::from(format!("/tmp/knot_tip_{}_{}", std::process::id(), id));
    let _ = std::fs::remove_dir_all(&path);
    
    let tip_hash = [0x99u8; 32];
    
    // Write tip
    {
        let db = ChainDB::open(&path).unwrap();
        db.set_tip(&tip_hash).unwrap();
        db.flush().unwrap();
    }
    
    // Verify after reopen
    {
        let db = ChainDB::open(&path).unwrap();
        assert_eq!(db.get_tip().unwrap(), Some(tip_hash));
    }
    
    let _ = std::fs::remove_dir_all(&path);
}
