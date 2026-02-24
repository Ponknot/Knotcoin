// Data Structures: Block
use super::transaction::Transaction;
use crate::crypto::hash::hash_sha3_256;
use crate::crypto::keys::ADDRESS_BYTES;

pub const BLOCK_HEADER_BYTES: usize = 148;
pub const BASE_BLOCK_SIZE_KB: usize = 50;
pub const MAX_BLOCK_SIZE_KB: usize = 500;
pub const TARGET_BLOCK_TIME_SEC: u64 = 60;

/// Strict adherence to Section 4: Block Header (148 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed(1))]
pub struct BlockHeader {
    pub version: [u8; 4],                   // 4 bytes
    pub previous_hash: [u8; 32],            // 32 bytes (SHA3-256)
    pub merkle_root: [u8; 32],              // 32 bytes (SHA3-256)
    pub timestamp: [u8; 4],                 // 4 bytes (u32 Unix timestamp)
    pub difficulty_target: [u8; 32],        // 32 bytes
    pub nonce: [u8; 8],                     // 8 bytes (u64 PONC puzzle nonce)
    pub block_height: [u8; 4],              // 4 bytes (u32 height)
    pub miner_address: [u8; ADDRESS_BYTES], // 32 bytes
}

impl BlockHeader {
    /// Computes the definitive Block Hash using SHA3-256
    pub fn hash(&self) -> [u8; 32] {
        let mut buffer = Vec::with_capacity(BLOCK_HEADER_BYTES);
        buffer.extend_from_slice(&self.version);
        buffer.extend_from_slice(&self.previous_hash);
        buffer.extend_from_slice(&self.merkle_root);
        buffer.extend_from_slice(&self.timestamp);
        buffer.extend_from_slice(&self.difficulty_target);
        buffer.extend_from_slice(&self.nonce);
        buffer.extend_from_slice(&self.block_height);
        buffer.extend_from_slice(&self.miner_address);

        hash_sha3_256(&buffer)
    }
}

/// A Full Block containing the header and ordered transactions.
#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    // The total size of the block (header + all txs) must be between 50KB and 500KB.
    // The first transaction must always be the Coinbase Transaction.
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Compute the Merkle Root using SHA3-256.
    pub fn compute_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        if transactions.is_empty() {
            return [0u8; 32];
        }

        let mut current_level: Vec<[u8; 32]> = transactions.iter().map(|tx| tx.txid()).collect();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let mut hasher_input = Vec::new();
                hasher_input.extend_from_slice(&chunk[0]);
                if chunk.len() == 2 {
                    hasher_input.extend_from_slice(&chunk[1]);
                } else {
                    // Duplicate last element if odd number
                    hasher_input.extend_from_slice(&chunk[0]);
                }
                next_level.push(hash_sha3_256(&hasher_input));
            }
            current_level = next_level;
        }

        current_level[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        // Assert that the serialized struct precisely matches the 148 byte requirement
        let header = BlockHeader {
            version: [0u8; 4],
            previous_hash: [0u8; 32],
            merkle_root: [0u8; 32],
            timestamp: [0u8; 4],
            difficulty_target: [0u8; 32],
            nonce: [0u8; 8],
            block_height: [0u8; 4],
            miner_address: [1u8; 32],
        };

        // Manual count to enforce whitepaper strictness (Section 4)
        let size = 4 + 32 + 32 + 4 + 32 + 8 + 4 + 32;
        assert_eq!(size, BLOCK_HEADER_BYTES);

        // Assert the struct in memory is exactly 148 bytes (no padding)
        assert_eq!(std::mem::size_of::<BlockHeader>(), BLOCK_HEADER_BYTES);

        let hash = header.hash();
        assert_eq!(hash.len(), 32);
    }
}
