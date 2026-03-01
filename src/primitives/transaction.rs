// Data Structures: Transaction
use crate::crypto::hash::hash_sha3_256;
use crate::crypto::keys::ADDRESS_BYTES;
use crate::crypto::dilithium::{PublicKey, Signature};
use crate::node::db_common::StoredTransaction;

pub const KNOTS_PER_KOT: u64 = 100_000_000;
pub const MIN_FEE_KNOTS: u64 = 1;

/// Strict adherence to Section 3 of Knotcoin Whitepaper
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    // Standard Fields
    pub version: u8, // Protocol version (0x01)
    pub sender_address: [u8; ADDRESS_BYTES],
    pub sender_pubkey: PublicKey,
    pub recipient_address: [u8; ADDRESS_BYTES],
    pub amount: u64, // In knots
    pub fee: u64,    // In knots
    pub nonce: u64,
    pub timestamp: u64, // Unix timestamp in seconds

    // Optional Registration Field (only for nonce 1)
    pub referrer_address: Option<[u8; ADDRESS_BYTES]>,

    // Optional Governance Field
    // If set, this transaction is a signaling vote or proposal.
    pub governance_data: Option<[u8; 32]>,

    pub signature: Signature,
}

impl Transaction {
    /// Computes the SHA3-256 hash of the transaction (without signature)
    pub fn signing_hash(&self) -> [u8; 32] {
        let mut buffer = Vec::new();
        buffer.push(self.version);
        buffer.extend_from_slice(&self.sender_address);
        buffer.extend_from_slice(&self.sender_pubkey.0);
        buffer.extend_from_slice(&self.recipient_address);
        buffer.extend_from_slice(&self.amount.to_le_bytes());
        buffer.extend_from_slice(&self.fee.to_le_bytes());
        buffer.extend_from_slice(&self.nonce.to_le_bytes());
        buffer.extend_from_slice(&self.timestamp.to_le_bytes());

        if let Some(ref_addr) = self.referrer_address {
            buffer.extend_from_slice(&ref_addr);
        }
        if let Some(gov_data) = self.governance_data {
            buffer.extend_from_slice(&gov_data);
        }

        hash_sha3_256(&buffer)
    }

    /// Computes the definitive Transaction ID (SHA3-256 of the FULL signed transaction)
    /// Prevents malleability.
    pub fn txid(&self) -> [u8; 32] {
        let mut buffer = self.signing_hash().to_vec();
        buffer.extend_from_slice(&self.signature.0);
        hash_sha3_256(&buffer)
    }

    /// Validates internal structural constraints. Does NOT validate state.
    pub fn is_structurally_valid(&self) -> bool {
        // 1. Minimum fee check
        if self.fee < MIN_FEE_KNOTS {
            return false;
        }

        // 2. Amount must be positive, UNLESS it is:
        //    - a governance signaling transaction, OR
        //    - a referral registration transaction (nonce==1, referrer set, self-recipient)
        if self.amount == 0 {
            let is_governance_signal = self.governance_data.is_some();
            let is_referral_registration = self.nonce == 1
                && self.referrer_address.is_some()
                && self.recipient_address == self.sender_address;

            if !is_governance_signal && !is_referral_registration {
                return false;
            }
        }

        // Catch arithmetic DoS attacks
        if self.amount.checked_add(self.fee).is_none() {
            return false;
        }

        // 3. Sender pubkey must match claimed address
        let derived_addr = crate::crypto::keys::derive_address(&self.sender_pubkey);
        if derived_addr != self.sender_address {
            return false;
        }

        // 4. Registration rules
        if self.nonce > 1 && self.referrer_address.is_some() {
            return false; // Referrer only allowed on first outbound txn
        }

        // 5. Signature verification
        let msg = self.signing_hash();
        if !crate::crypto::dilithium::verify(&msg, &self.signature, &self.sender_pubkey) {
            return false;
        }

        true
    }
}

impl TryFrom<&StoredTransaction> for Transaction {
    type Error = &'static str;

    fn try_from(st: &StoredTransaction) -> Result<Self, Self::Error> {
        let mut pk = [0u8; 1952];
        if st.sender_pubkey.len() != 1952 {
            return Err("invalid public key length");
        }
        pk.copy_from_slice(&st.sender_pubkey);

        let mut sig = [0u8; 3309];
        if st.signature.len() != 3309 {
            return Err("invalid signature length");
        }
        sig.copy_from_slice(&st.signature);

        Ok(Transaction {
            version: st.version,
            sender_address: st.sender_address,
            sender_pubkey: PublicKey(pk),
            recipient_address: st.recipient_address,
            amount: st.amount,
            fee: st.fee,
            nonce: st.nonce,
            timestamp: st.timestamp,
            referrer_address: st.referrer_address,
            governance_data: st.governance_data,
            signature: Signature(sig),
        })
    }
}

pub struct CoinbaseTransaction {
    pub recipient_address: [u8; ADDRESS_BYTES],
    pub amount: u64,         // Total reward (base + fees)
    pub referral_bonus: u64, // Minted explicitly for the referrer (5%)
    pub block_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::dilithium;

    // builds a fully valid, properly signed transaction
    fn mock_tx() -> Transaction {
        let (pk, sk) = dilithium::generate_keypair(&[0u8; 64]);
        let addr = crate::crypto::keys::derive_address(&pk);

        let mut tx = Transaction {
            version: 1,
            sender_address: addr,
            sender_pubkey: pk,
            recipient_address: [2u8; 32],
            amount: 50 * KNOTS_PER_KOT,
            fee: MIN_FEE_KNOTS,
            nonce: 2,
            timestamp: 1700000000,
            referrer_address: None,
            governance_data: None,
            signature: dilithium::Signature([0u8; 3309]), // placeholder
        };

        // sign the tx properly
        let msg = tx.signing_hash();
        tx.signature = dilithium::sign(&msg, &sk);
        tx
    }

    #[test]
    fn test_valid_tx() {
        let tx = mock_tx();
        assert!(tx.is_structurally_valid());
    }

    #[test]
    fn test_invalid_fee() {
        let mut tx = mock_tx();
        tx.fee = 0;
        // re-sign not needed — fee=0 fails before sig check
        assert!(!tx.is_structurally_valid());
    }

    #[test]
    fn test_late_referrer() {
        let mut tx = mock_tx();
        tx.nonce = 2;
        tx.referrer_address = Some([3u8; 32]);
        assert!(!tx.is_structurally_valid());
    }

    #[test]
    fn test_wrong_signature_rejected() {
        let mut tx = mock_tx();
        tx.signature.0[0] ^= 0xFF;
        assert!(!tx.is_structurally_valid());
    }

    #[test]
    fn test_wrong_pubkey_rejected() {
        let mut tx = mock_tx();
        tx.sender_address = [1u8; 32]; // Doesn't match pubkey
        assert!(!tx.is_structurally_valid());
    }

    #[test]
    fn test_zero_amount_rejected() {
        let mut tx = mock_tx();
        tx.amount = 0;
        // Re-sign
        let (pk, sk) = dilithium::generate_keypair(&[0u8; 64]);
        tx.sender_pubkey = pk;
        tx.sender_address = crate::crypto::keys::derive_address(&tx.sender_pubkey);
        let msg = tx.signing_hash();
        tx.signature = dilithium::sign(&msg, &sk);
        
        assert!(!tx.is_structurally_valid());
    }
}
