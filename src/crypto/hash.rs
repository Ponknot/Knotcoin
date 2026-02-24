// Cryptographic Hashing Wrappers
use sha2::{Digest, Sha512};
use sha3::Sha3_256;

/// SHA-512: Used for address derivation
pub fn hash_sha512(data: &[u8]) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA3-256: Used exclusively for Proof of Work (PONC) and Merkle Tree hashing
pub fn hash_sha3_256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// SHA3-256 Concat: Hashes a then b without allocating a temporary Vec
pub fn hash_sha3_256_concat(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(a);
    hasher.update(b);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha512_length() {
        let hash = hash_sha512(b"knotcoin");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_sha3_256_length() {
        let hash = hash_sha3_256(b"knotcoin");
        assert_eq!(hash.len(), 32);
    }
}
