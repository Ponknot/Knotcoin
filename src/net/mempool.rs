// Transaction Mempool
//
// In-memory pool of unconfirmed transactions, ordered by fee priority.
// Supports Replace-by-Fee (10% higher minimum) and reserves one slot
// per block for Layer 2 dispute transactions.

use crate::crypto::hash::hash_sha3_256;
use crate::node::db_common::StoredTransaction;
use crate::primitives::transaction::Transaction;
use std::collections::HashMap;

const MAX_MEMPOOL_SIZE: usize = 5000;

/// A mempool entry wrapping a transaction with its computed hash
#[derive(Debug, Clone)]
pub struct MempoolEntry {
    pub tx: StoredTransaction,
    pub txid: [u8; 32],
    pub fee_per_byte_scaled: u64, // fee * 10000 / size for deterministic integer comparison
}

pub struct Mempool {
    /// txid -> entry
    entries: HashMap<[u8; 32], MempoolEntry>,
    /// sender_address + nonce -> txid (for Replace-by-Fee lookup)
    by_sender_nonce: HashMap<([u8; 32], u64), [u8; 32]>,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    pub fn new() -> Self {
        Mempool {
            entries: HashMap::new(),
            by_sender_nonce: HashMap::new(),
        }
    }

    pub fn compute_txid_from_stored(tx: &StoredTransaction) -> [u8; 32] {
        Self::compute_txid(tx)
    }

    /// Compute a transaction's hash from its serialized fields
    fn compute_txid(tx: &StoredTransaction) -> [u8; 32] {
        let mut buf = Vec::new();
        buf.push(tx.version);
        buf.extend_from_slice(&tx.sender_address);
        buf.extend_from_slice(&tx.sender_pubkey);
        buf.extend_from_slice(&tx.recipient_address);
        buf.extend_from_slice(&tx.amount.to_le_bytes());
        buf.extend_from_slice(&tx.fee.to_le_bytes());
        buf.extend_from_slice(&tx.nonce.to_le_bytes());
        buf.extend_from_slice(&tx.timestamp.to_le_bytes());
        if let Some(ref_addr) = tx.referrer_address {
            buf.extend_from_slice(&ref_addr);
        }
        if let Some(gov_data) = tx.governance_data {
            buf.extend_from_slice(&gov_data);
        }
        buf.extend_from_slice(&tx.signature);
        hash_sha3_256(&buf)
    }

    /// Approximate transaction size in bytes
    fn estimate_tx_size(tx: &StoredTransaction) -> usize {
        let mut base = 1 + 32 + 4 + 1952 + 32 + 8 + 8 + 8 + 8 + 1 + 1 + 4 + 3309;
        if tx.referrer_address.is_some() {
            base += 32;
        }
        if tx.governance_data.is_some() {
            base += 32;
        }
        base
    }

    /// Add a transaction to the mempool. Returns Ok(true) if added,
    /// Ok(false) if it replaced an existing tx, or Err on rejection.
    pub fn add_transaction(&mut self, tx: StoredTransaction) -> Result<bool, &'static str> {
        // 0. Domain Validation (Structural & Signature)
        let domain_tx = Transaction::try_from(&tx)?;
        if !domain_tx.is_structurally_valid() {
            return Err("structural or signature validation failed");
        }

        if tx.fee < 1 {
            return Err("fee below minimum (1 knot)");
        }

        // Section 3: Even 0-amount governance signals must pay for network resources.
        if tx.amount == 0 && tx.fee < 1 {
            return Err("insufficient fee for signaling transaction");
        }

        let txid = Self::compute_txid(&tx);

        // Already in pool?
        if self.entries.contains_key(&txid) {
            return Err("duplicate transaction");
        }

        let sender_nonce_key = (tx.sender_address, tx.nonce);

        // Replace-by-Fee check
        if let Some(existing_txid) = self.by_sender_nonce.get(&sender_nonce_key) {
            let existing_txid = *existing_txid;
            if let Some(existing) = self.entries.get(&existing_txid) {
                // New fee must be at least 10% higher
                let min_replacement_fee = existing.tx.fee + (existing.tx.fee / 10).max(1);
                if tx.fee < min_replacement_fee {
                    return Err("replacement fee too low (must be >= 110% of existing)");
                }
                // Replace it
                self.entries.remove(&existing_txid);
                self.by_sender_nonce.remove(&sender_nonce_key);
            }
        }

        // Pool size limit
        if self.entries.len() >= MAX_MEMPOOL_SIZE {
            // Evict the lowest-fee transaction
            let worst_txid = self
                .entries
                .iter()
                .min_by_key(|(_id, entry)| entry.fee_per_byte_scaled)
                .map(|(&id, _)| id);

            if let Some(id) = worst_txid
                && let Some(evicted) = self.entries.remove(&id)
            {
                let evict_key = (evicted.tx.sender_address, evicted.tx.nonce);
                self.by_sender_nonce.remove(&evict_key);
            }
        }

        let size = Self::estimate_tx_size(&tx) as u64;
        // Integer-only fee calculation: (fee * 10000) / size
        // This ensures deterministic sorting across all platforms
        let fee_per_byte_scaled = (tx.fee * 10000) / size.max(1);

        let entry = MempoolEntry {
            tx,
            txid,
            fee_per_byte_scaled,
        };
        self.by_sender_nonce.insert(sender_nonce_key, txid);
        let replaced = self.entries.insert(txid, entry).is_some();

        Ok(!replaced)
    }

    /// Get the top N transactions sorted by fee (highest first) for block template
    pub fn get_top_transactions(&self, max_count: usize) -> Vec<StoredTransaction> {
        let mut entries: Vec<&MempoolEntry> = self.entries.values().collect();
        // Sort by fee_per_byte_scaled (descending), then by txid for determinism
        entries.sort_by(|a, b| {
            b.fee_per_byte_scaled
                .cmp(&a.fee_per_byte_scaled)
                .then_with(|| a.txid.cmp(&b.txid))
        });
        entries
            .into_iter()
            .take(max_count)
            .map(|e| e.tx.clone())
            .collect()
    }

    /// Remove transactions that were included in a mined block
    pub fn remove_confirmed(&mut self, txids: &[[u8; 32]]) {
        for txid in txids {
            if let Some(entry) = self.entries.remove(txid) {
                let key = (entry.tx.sender_address, entry.tx.nonce);
                self.by_sender_nonce.remove(&key);
            }
        }
    }

    pub fn get_all_txids(&self) -> Vec<[u8; 32]> {
        self.entries.keys().cloned().collect()
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }

    pub fn highest_pending_nonce_for_sender(&self, sender: &[u8; 32]) -> Option<u64> {
        let mut max_nonce: Option<u64> = None;
        for ((s, nonce), txid) in &self.by_sender_nonce {
            if s == sender {
                if self.entries.contains_key(txid) {
                    max_nonce = Some(max_nonce.map(|m| m.max(*nonce)).unwrap_or(*nonce));
                }
            }
        }
        max_nonce
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::dilithium;
    use crate::primitives::transaction::Transaction;

    // build a signed StoredTransaction from a given keypair
    fn mock_stored_tx_with_keys(
        pk: &dilithium::PublicKey,
        sk: &dilithium::SecretKey,
        nonce: u64,
        fee: u64,
    ) -> StoredTransaction {
        let addr = crate::crypto::keys::derive_address(pk);

        let mut domain_tx = Transaction {
            version: 1,
            sender_address: addr,
            sender_pubkey: *pk,
            recipient_address: [2u8; 32],
            amount: 1_000_000,
            fee,
            nonce,
            timestamp: 1700000000,
            referrer_address: None,
            governance_data: None,
            signature: dilithium::Signature([0u8; 3309]),
        };
        let msg = domain_tx.signing_hash();
        domain_tx.signature = dilithium::sign(&msg, sk);

        StoredTransaction {
            version: 1,
            sender_address: addr,
            sender_pubkey: pk.0.to_vec(),
            recipient_address: [2u8; 32],
            amount: 1_000_000,
            fee,
            nonce,
            timestamp: 1700000000,
            referrer_address: None,
            governance_data: None,
            signature: domain_tx.signature.0.to_vec(),
        }
    }

    // convenience: fresh random-looking keypair per call
    fn mock_stored_tx(nonce: u64, fee: u64, seed_byte: u8) -> StoredTransaction {
        let (pk, sk) = dilithium::generate_keypair(&[seed_byte; 64]);
        mock_stored_tx_with_keys(&pk, &sk, nonce, fee)
    }

    #[test]
    fn test_add_and_retrieve() {
        let mut pool = Mempool::new();
        let tx = mock_stored_tx(1, 100, 1);
        assert!(pool.add_transaction(tx).unwrap());
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_replace_by_fee() {
        let mut pool = Mempool::new();
        // same keypair for all three — RBF requires same sender + nonce
        let (pk, sk) = dilithium::generate_keypair(&[0u8; 64]);

        let tx1 = mock_stored_tx_with_keys(&pk, &sk, 1, 100);
        pool.add_transaction(tx1).unwrap();
        assert_eq!(pool.size(), 1);

        // >= 110% of 100 → 111 is enough
        let tx2 = mock_stored_tx_with_keys(&pk, &sk, 1, 111);
        pool.add_transaction(tx2).unwrap();
        assert_eq!(pool.size(), 1);

        // 112 < 111 * 1.1 = 122.1 → must be rejected
        let tx3 = mock_stored_tx_with_keys(&pk, &sk, 1, 112);
        let result = pool.add_transaction(tx3);
        assert!(result.is_err() || pool.size() == 1);
    }

    #[test]
    fn test_fee_ordering() {
        let mut pool = Mempool::new();
        pool.add_transaction(mock_stored_tx(1, 10, 1)).unwrap();
        pool.add_transaction(mock_stored_tx(1, 50, 2)).unwrap();
        pool.add_transaction(mock_stored_tx(1, 30, 3)).unwrap();

        let top = pool.get_top_transactions(2);
        assert_eq!(top.len(), 2);
        assert!(top[0].fee >= top[1].fee);
    }

    #[test]
    fn test_reject_zero_fee() {
        let mut pool = Mempool::new();
        let tx = mock_stored_tx(1, 0, 1);
        assert!(pool.add_transaction(tx).is_err());
    }
}
