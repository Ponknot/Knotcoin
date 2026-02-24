// Key Derivation and Address Management
use crate::crypto::dilithium::PublicKey;
use crate::crypto::wordlist::ENGLISH;
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2;

use sha2::{Digest, Sha256, Sha512};

pub const ADDRESS_BYTES: usize = 32;

/// Derives a Knotcoin Address from a Dilithium3 Public Key
/// Rule: address = first 32 bytes of SHA-512(public_key)
pub fn derive_address(pk: &PublicKey) -> [u8; ADDRESS_BYTES] {
    let hash = super::hash::hash_sha512(&pk.0);
    let mut addr = [0u8; ADDRESS_BYTES];
    addr.copy_from_slice(&hash[0..ADDRESS_BYTES]);
    addr
}

#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    #[error("Invalid address prefix: must start with KOT1")]
    InvalidPrefix,
    #[error("Invalid address encoding")]
    InvalidEncoding,
    #[error("Invalid address length")]
    InvalidLength,
    #[error("Invalid address checksum")]
    InvalidChecksum,
}

/// Encodes an address into the human-readable Base32 string
/// Format: KOT1<base32_address><4-byte_checksum>
pub fn encode_address_string(addr: &[u8; ADDRESS_BYTES]) -> String {
    let b32 = data_encoding::BASE32_NOPAD.encode(addr);

    // Checksum: sha3_256(sha3_256("KOT1" + address_bytes))[0..4]
    let prefix = b"KOT1";
    let mut payload = Vec::with_capacity(prefix.len() + addr.len());
    payload.extend_from_slice(prefix);
    payload.extend_from_slice(addr);

    let hash1 = super::hash::hash_sha3_256(&payload);
    let hash2 = super::hash::hash_sha3_256(&hash1);

    let checksum = data_encoding::BASE32_NOPAD.encode(&hash2[0..4]);

    format!("KOT1{}{}", b32, checksum)
}

/// Decodes a human-readable KOT1 address back to raw bytes.
pub fn decode_address_string(s: &str) -> Result<[u8; 32], AddressError> {
    if !s.starts_with("KOT1") {
        return Err(AddressError::InvalidPrefix);
    }
    
    let body = &s[4..];
    if body.len() < 8 {
        return Err(AddressError::InvalidLength);
    }

    let (addr_part, _checksum_part) = body.split_at(body.len() - 7); 
    
    let addr_bytes = data_encoding::BASE32_NOPAD
        .decode(addr_part.as_bytes())
        .map_err(|_| AddressError::InvalidEncoding)?;
        
    if addr_bytes.len() != 32 {
        return Err(AddressError::InvalidLength);
    }

    let mut addr = [0u8; 32];
    addr.copy_from_slice(&addr_bytes);

    let expected = encode_address_string(&addr);
    if expected == s { 
        Ok(addr) 
    } else { 
        Err(AddressError::InvalidChecksum)
    }
}

/// Generates a new cryptographically secure 24-word BIP-39 mnemonic.
pub fn generate_mnemonic() -> String {
    let mut entropy = [0u8; 32]; // 32 bytes = 256 bits for 24 words
    getrandom::getrandom(&mut entropy).expect("RNG failure");

    // Calculate checksum (first 8 bits of SHA256 hash for 256-bit entropy)
    let hash = Sha256::digest(entropy);
    let checksum_byte = hash[0];

    // Merge entropy and checksum into bits
    let mut bits = Vec::with_capacity(264); // 256 + 8 = 264 bits
    for byte in entropy {
        for i in (0..8).rev() {
            bits.push((byte >> i) & 1);
        }
    }
    // For 24 words, we need 8 bits of checksum
    for i in (0..8).rev() {
        bits.push((checksum_byte >> i) & 1);
    }

    // Map bits to words (264 bits / 11 bits per word = 24 words)
    let mut words = Vec::new();
    for chunk in bits.chunks(11) {
        let mut index = 0usize;
        for (i, bit) in chunk.iter().enumerate() {
            if *bit == 1 {
                index |= 1 << (10 - i);
            }
        }
        words.push(ENGLISH[index]);
    }
    words.join(" ")
}

/// Derives the master seed from a BIP-39 mnemonic string
pub fn derive_master_seed(mnemonic: &str, passphrase: &str) -> [u8; 64] {
    // Step 1: PBKDF2
    let salt = format!("mnemonic{}", passphrase);
    let mut bip39_seed = [0u8; 64];
    pbkdf2::<Hmac<Sha512>>(mnemonic.as_bytes(), salt.as_bytes(), 2048, &mut bip39_seed)
        .expect("PBKDF2 failed");

    // Step 2: Knotcoin Master Key
    let mut mac =
        Hmac::<Sha512>::new_from_slice(b"Knotcoin seed v1").expect("HMAC can take key of any size");
    mac.update(&bip39_seed);
    let result = mac.finalize();

    let mut master_key = [0u8; 64];
    master_key.copy_from_slice(&result.into_bytes());
    master_key
}

/// Derives an account-specific key (Account 0 is primary)
pub fn derive_account_seed(master_seed: &[u8; 64], account_index: u64) -> [u8; 64] {
    let mut mac =
        Hmac::<Sha512>::new_from_slice(b"Knotcoin account").expect("HMAC can take key of any size");
    mac.update(master_seed);
    mac.update(&account_index.to_be_bytes());
    let result = mac.finalize();

    let mut account_key = [0u8; 64];
    account_key.copy_from_slice(&result.into_bytes());
    account_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_derivation() {
        let pk = PublicKey([1u8; 1952]);
        let addr = derive_address(&pk);
        assert_eq!(addr.len(), 32);

        let addr_str = encode_address_string(&addr);
        assert!(addr_str.starts_with("KOT1"), "Address must start with KOT1 (uppercase)");

        let decoded = decode_address_string(&addr_str).unwrap();
        assert_eq!(decoded, addr);
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        let m = generate_mnemonic();
        assert_eq!(m.split_whitespace().count(), 24, "must generate 24 words");

        let s = derive_master_seed(&m, "");
        assert_eq!(s.len(), 64);
        
        // Test determinism: same mnemonic produces same seed
        let s2 = derive_master_seed(&m, "");
        assert_eq!(s, s2, "same mnemonic must produce same seed");
    }
}
