// Genesis block definition
//
// The genesis block has no previous hash, no transactions, and the minimum
// Phase 1 reward. Miner address is set to the creator's wallet address.
//
// CRITICAL: This address must be a valid KOT1 address derived from an actual
// Dilithium3 keypair. The creator must have the corresponding private key to
// spend the genesis reward.

use crate::node::db_common::StoredBlock;

/// Mainnet genesis timestamp: Feb 25 2026 00:00:00 UTC
/// IMPORTANT: This should be set to NOW or a few minutes in the future before mining
const MAINNET_GENESIS_TIMESTAMP: u32 = 1771545600;

/// Mainnet genesis difficulty: easy for the first block.
fn mainnet_genesis_target() -> [u8; 32] {
    let mut target = [0xFF; 32];
    target[0] = 0x7F; // Just slightly below max
    target
}

/// Genesis miner address
/// CRITICAL: This must be replaced with the actual wallet address before mining.
/// Current placeholder will be replaced with real address from creator's wallet.
/// 
/// To generate: 
/// 1. Create wallet with `knotcoind wallet create`
/// 2. Get address with `knotcoind wallet address`
/// 3. Convert KOT1... string to raw 32 bytes
/// 4. Update this constant
/// 
/// DO NOT MINE GENESIS UNTIL THIS IS SET TO A REAL ADDRESS
fn genesis_miner_address() -> [u8; 32] {
    // PLACEHOLDER - MUST BE REPLACED BEFORE MINING
    // This is intentionally an invalid address to prevent accidental mining
    [0xFFu8; 32]
}

pub fn create_genesis_block() -> StoredBlock {
    StoredBlock {
        version: [1, 0, 0, 0],
        previous_hash: [0u8; 32],
        merkle_root: [0u8; 32],
        timestamp: MAINNET_GENESIS_TIMESTAMP.to_le_bytes(),
        difficulty_target: mainnet_genesis_target(),
        nonce: [0u8; 8], // Will be filled in after mining
        block_height: 0u32.to_le_bytes(),
        miner_address: genesis_miner_address(),
        tx_data: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block_structure() {
        let genesis = create_genesis_block();
        assert_eq!(genesis.previous_hash, [0u8; 32]);
        // Genesis miner address must NOT be all zeros (would burn reward)
        assert_ne!(genesis.miner_address, [0u8; 32], "genesis miner address cannot be zero"); 
        assert_eq!(u32::from_le_bytes(genesis.block_height), 0);
        assert_eq!(u32::from_le_bytes(genesis.timestamp), MAINNET_GENESIS_TIMESTAMP);
        assert!(genesis.tx_data.is_empty());
        
        // Warn if still using placeholder
        if genesis.miner_address == [0xFFu8; 32] {
            eprintln!("WARNING: Genesis miner address is still placeholder [0xFF; 32]");
            eprintln!("MUST be replaced with real wallet address before mining!");
        }
    }
}
