// Dilithium3 Digital Signatures — NIST FIPS 204
//
// Dilithium is a lattice-based signature scheme standardized by NIST in 2024.
// We use the Dilithium3 parameter set, which provides NIST Security Level 3
// (equivalent to AES-192 or SHA-384 against quantum computers).
//
// Key sizes (pqcrypto_dilithium reference implementation):
//   Public key  : 1,952 bytes
//   Secret key  : 4,032 bytes
//   Signature   : 3,309 bytes
//
// NOTE: Key generation is non-deterministic (uses OS randomness).
// Wallet identity stability across restarts is achieved via wallet_keys.json
// persistence on disk. The mnemonic serves as a wallet identifier (hash-keyed
// lookup) rather than a deterministic seed for Dilithium keygen.

use pqcrypto_dilithium::dilithium3;
use pqcrypto_traits::sign::{
    DetachedSignature as PqDetachedSig, PublicKey as PqPk, SecretKey as PqSk,
};
use fips204::traits::SerDes;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

pub const DILITHIUM3_PUBKEY_BYTES: usize = 1952;
pub const DILITHIUM3_PRIVKEY_BYTES: usize = 4032;
pub const DILITHIUM3_SIG_BYTES: usize = 3309;

#[derive(Clone, Copy)]
pub struct PublicKey(pub [u8; DILITHIUM3_PUBKEY_BYTES]);

impl serde::Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != DILITHIUM3_PUBKEY_BYTES {
            return Err(serde::de::Error::custom("invalid public key length"));
        }
        let mut arr = [0u8; DILITHIUM3_PUBKEY_BYTES];
        arr.copy_from_slice(&bytes);
        Ok(PublicKey(arr))
    }
}

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({} bytes)", DILITHIUM3_PUBKEY_BYTES)
    }
}

#[derive(Clone)]
pub struct SecretKey(pub [u8; DILITHIUM3_PRIVKEY_BYTES]);

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretKey([REDACTED])")
    }
}

#[derive(Clone)]
pub struct Signature(pub [u8; DILITHIUM3_SIG_BYTES]);

impl serde::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != DILITHIUM3_SIG_BYTES {
            return Err(serde::de::Error::custom("invalid signature length"));
        }
        let mut arr = [0u8; DILITHIUM3_SIG_BYTES];
        arr.copy_from_slice(&bytes);
        Ok(Signature(arr))
    }
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature({} bytes)", DILITHIUM3_SIG_BYTES)
    }
}

/// Generates a Dilithium3 keypair DETERMINISTICALLY from a 64-byte seed.
/// This ensures the same mnemonic always produces the same address and keys even if wallet_keys.json is lost.
/// 
/// Uses ChaCha20 RNG seeded with the 64-byte input for cryptographically secure deterministic generation.
pub fn generate_keypair(seed: &[u8; 64]) -> (PublicKey, SecretKey) {
    // Create deterministic RNG from seed using ChaCha20 (cryptographically secure)
    let mut seed_32 = [0u8; 32];
    seed_32.copy_from_slice(&seed[0..32]); // Use first 32 bytes as ChaCha20 seed
    let mut rng = ChaCha20Rng::from_seed(seed_32);
    
    // Generate deterministic keys matching ML-DSA-65 (Dilithium3)
    let (fips_pk, fips_sk) = fips204::ml_dsa_65::try_keygen_with_rng(&mut rng)
        .expect("Deterministic keygen failed");

    let mut pk = [0u8; DILITHIUM3_PUBKEY_BYTES];
    let mut sk = [0u8; DILITHIUM3_PRIVKEY_BYTES];
    
    pk.copy_from_slice(&fips_pk.into_bytes());
    sk.copy_from_slice(&fips_sk.into_bytes());

    (PublicKey(pk), SecretKey(sk))
}

/// Signs a message with a Dilithium3 detached signature.
pub fn sign(message: &[u8], sk: &SecretKey) -> Signature {
    let pq_sk = dilithium3::SecretKey::from_bytes(&sk.0)
        .expect("secret key bytes are always valid");
    let det_sig = dilithium3::detached_sign(message, &pq_sk);

    let mut sig = [0u8; DILITHIUM3_SIG_BYTES];
    sig.copy_from_slice(det_sig.as_bytes());
    Signature(sig)
}

/// Verifies a Dilithium3 detached signature.
/// Returns false on any malformed input — never panics.
pub fn verify(message: &[u8], sig: &Signature, pk: &PublicKey) -> bool {
    let pq_pk = match dilithium3::PublicKey::from_bytes(&pk.0) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let pq_sig = match dilithium3::DetachedSignature::from_bytes(&sig.0) {
        Ok(s) => s,
        Err(_) => return false,
    };
    dilithium3::verify_detached_signature(&pq_sig, message, &pq_pk).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let (pk, sk) = generate_keypair(&[0u8; 64]);
        let msg = b"knotcoin genesis";
        let sig = sign(msg, &sk);
        assert!(verify(msg, &sig, &pk), "valid signature must verify");
    }

    #[test]
    fn test_wrong_message_fails() {
        let (pk, sk) = generate_keypair(&[0u8; 64]);
        let sig = sign(b"correct message", &sk);
        assert!(!verify(b"wrong message", &sig, &pk));
    }

    #[test]
    fn test_wrong_key_fails() {
        let (_pk1, sk1) = generate_keypair(&[0u8; 64]);
        let (pk2, _sk2) = generate_keypair(&[1u8; 64]);
        let sig = sign(b"test", &sk1);
        assert!(!verify(b"test", &sig, &pk2));
    }

    #[test]
    fn test_signature_size() {
        let (_pk, sk) = generate_keypair(&[0u8; 64]);
        let sig = sign(b"knotcoin", &sk);
        assert_eq!(sig.0.len(), DILITHIUM3_SIG_BYTES);
        assert_eq!(DILITHIUM3_SIG_BYTES, 3309);
    }

    #[test]
    fn test_corrupted_signature_rejected() {
        let (pk, sk) = generate_keypair(&[0u8; 64]);
        let msg = b"knotcoin";
        let mut sig = sign(msg, &sk);
        sig.0[100] ^= 0xFF;
        assert!(!verify(msg, &sig, &pk), "corrupted signature must fail");
    }

    #[test]
    fn test_key_sizes() {
        let (pk, sk) = generate_keypair(&[42u8; 64]);
        assert_eq!(pk.0.len(), DILITHIUM3_PUBKEY_BYTES);
        assert_eq!(sk.0.len(), DILITHIUM3_PRIVKEY_BYTES);
        assert_eq!(DILITHIUM3_PUBKEY_BYTES, 1952);
        assert_eq!(DILITHIUM3_PRIVKEY_BYTES, 4032);
    }

    #[test]
    fn test_deterministic_keygen() {
        // Same seed must produce same keys
        let seed = [42u8; 64];
        let (pk1, sk1) = generate_keypair(&seed);
        let (pk2, sk2) = generate_keypair(&seed);
        
        assert_eq!(pk1.0, pk2.0, "Same seed must produce same public key");
        assert_eq!(sk1.0, sk2.0, "Same seed must produce same secret key");
        
        // Different seed must produce different keys
        let seed2 = [43u8; 64];
        let (pk3, _sk3) = generate_keypair(&seed2);
        assert_ne!(pk1.0, pk3.0, "Different seeds must produce different keys");
    }
}
