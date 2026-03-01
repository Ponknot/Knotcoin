// Wallet Keystore - Encrypted Storage for Dilithium3 Secret Keys
//
// Security Model:
// - User password → Argon2id → 32-byte encryption key
// - Secret key encrypted with AES-256-GCM
// - Fresh random 12-byte nonce per encryption
// - Authentication tag prevents tampering
// - Wrong password → authentication failure (no garbled output)

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, ParamsBuilder, Version};
use getrandom::getrandom;
use std::fs;
use std::path::Path;

// Argon2id parameters (OWASP recommendations for 2024+)
const ARGON2_M_COST: u32 = 65536; // 64 MB memory
const ARGON2_T_COST: u32 = 3; // 3 iterations
const ARGON2_P_COST: u32 = 4; // 4 parallelism

const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SECRET_KEY_LEN: usize = 4032; // Dilithium3 secret key size (NIST FIPS 204)

#[derive(Debug)]
pub enum KeystoreError {
    Io(std::io::Error),
    Crypto(&'static str),
    InvalidPassword,
    InvalidFormat,
}

impl std::fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeystoreError::Io(e) => write!(f, "I/O error: {}", e),
            KeystoreError::Crypto(s) => write!(f, "Crypto error: {}", s),
            KeystoreError::InvalidPassword => write!(f, "Invalid password"),
            KeystoreError::InvalidFormat => write!(f, "Invalid keystore format"),
        }
    }
}

impl std::error::Error for KeystoreError {}

impl From<std::io::Error> for KeystoreError {
    fn from(e: std::io::Error) -> Self {
        KeystoreError::Io(e)
    }
}

/// Encrypted keystore file format:
/// [32 bytes salt][12 bytes nonce][N bytes ciphertext (64 + 16 tag)]
pub struct EncryptedKeystore {
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    ciphertext: Vec<u8>, // encrypted secret key + auth tag
}

impl EncryptedKeystore {
    /// Encrypt a Dilithium3 secret key with a password
    pub fn encrypt(secret_key: &[u8; SECRET_KEY_LEN], password: &str) -> Result<Self, KeystoreError> {
        // Generate random salt
        let mut salt = [0u8; SALT_LEN];
        getrandom(&mut salt).map_err(|_| KeystoreError::Crypto("RNG failure"))?;

        // Derive encryption key from password using Argon2id
        let encryption_key = derive_key(password, &salt)?;

        // Generate random nonce (MUST be unique per encryption)
        let mut nonce = [0u8; NONCE_LEN];
        getrandom(&mut nonce).map_err(|_| KeystoreError::Crypto("RNG failure"))?;

        // Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new(&encryption_key.into());
        let nonce_obj = Nonce::from_slice(&nonce);
        
        let ciphertext = cipher
            .encrypt(nonce_obj, secret_key.as_ref())
            .map_err(|_| KeystoreError::Crypto("Encryption failed"))?;

        Ok(EncryptedKeystore {
            salt,
            nonce,
            ciphertext,
        })
    }

    /// Decrypt a Dilithium3 secret key with a password
    pub fn decrypt(&self, password: &str) -> Result<[u8; SECRET_KEY_LEN], KeystoreError> {
        // Derive encryption key from password
        let encryption_key = derive_key(password, &self.salt)?;

        // Decrypt with AES-256-GCM
        let cipher = Aes256Gcm::new(&encryption_key.into());
        let nonce_obj = Nonce::from_slice(&self.nonce);

        let plaintext = cipher
            .decrypt(nonce_obj, self.ciphertext.as_ref())
            .map_err(|_| KeystoreError::InvalidPassword)?; // Auth tag failure = wrong password

        if plaintext.len() != SECRET_KEY_LEN {
            return Err(KeystoreError::InvalidFormat);
        }

        let mut secret_key = [0u8; SECRET_KEY_LEN];
        secret_key.copy_from_slice(&plaintext);
        Ok(secret_key)
    }

    /// Save encrypted keystore to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), KeystoreError> {
        let mut data = Vec::with_capacity(SALT_LEN + NONCE_LEN + self.ciphertext.len());
        data.extend_from_slice(&self.salt);
        data.extend_from_slice(&self.nonce);
        data.extend_from_slice(&self.ciphertext);

        fs::write(path, data)?;
        Ok(())
    }

    /// Load encrypted keystore from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, KeystoreError> {
        let data = fs::read(path)?;

        if data.len() < SALT_LEN + NONCE_LEN + 16 {
            // Minimum: salt + nonce + empty ciphertext + auth tag
            return Err(KeystoreError::InvalidFormat);
        }

        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        salt.copy_from_slice(&data[0..SALT_LEN]);
        nonce.copy_from_slice(&data[SALT_LEN..SALT_LEN + NONCE_LEN]);
        let ciphertext = data[SALT_LEN + NONCE_LEN..].to_vec();

        Ok(EncryptedKeystore {
            salt,
            nonce,
            ciphertext,
        })
    }
}

/// Derive a 32-byte encryption key from password using Argon2id
fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<[u8; 32], KeystoreError> {
    let params = ParamsBuilder::new()
        .m_cost(ARGON2_M_COST)
        .t_cost(ARGON2_T_COST)
        .p_cost(ARGON2_P_COST)
        .build()
        .map_err(|_| KeystoreError::Crypto("Invalid Argon2 parameters"))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| KeystoreError::Crypto("Key derivation failed"))?;

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let secret_key = [42u8; 4032];
        let password = "correct horse battery staple";

        let keystore = EncryptedKeystore::encrypt(&secret_key, password).unwrap();
        let decrypted = keystore.decrypt(password).unwrap();

        assert_eq!(secret_key, decrypted);
    }

    #[test]
    fn test_wrong_password() {
        let secret_key = [42u8; 4032];
        let password = "correct password";

        let keystore = EncryptedKeystore::encrypt(&secret_key, password).unwrap();
        let result = keystore.decrypt("wrong password");

        assert!(matches!(result, Err(KeystoreError::InvalidPassword)));
    }

    #[test]
    fn test_different_nonces() {
        let secret_key = [42u8; 4032];
        let password = "test";

        let ks1 = EncryptedKeystore::encrypt(&secret_key, password).unwrap();
        let ks2 = EncryptedKeystore::encrypt(&secret_key, password).unwrap();

        // Same plaintext + password but different nonces → different ciphertexts
        assert_ne!(ks1.nonce, ks2.nonce);
        assert_ne!(ks1.ciphertext, ks2.ciphertext);

        // Both decrypt correctly
        assert_eq!(ks1.decrypt(password).unwrap(), secret_key);
        assert_eq!(ks2.decrypt(password).unwrap(), secret_key);
    }

    #[test]
    fn test_file_roundtrip() {
        let secret_key = [99u8; 4032];
        let password = "file test password";
        let path = "/tmp/knotcoin_keystore_test.dat";

        let keystore = EncryptedKeystore::encrypt(&secret_key, password).unwrap();
        keystore.save_to_file(path).unwrap();

        let loaded = EncryptedKeystore::load_from_file(path).unwrap();
        let decrypted = loaded.decrypt(password).unwrap();

        assert_eq!(secret_key, decrypted);

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_argon2_parameters() {
        // Compile-time verification of security parameters
        const _: () = assert!(ARGON2_M_COST >= 65536, "Memory cost too low");
        const _: () = assert!(ARGON2_T_COST >= 3, "Time cost too low");
        const _: () = assert!(ARGON2_P_COST >= 4, "Parallelism too low");
        
        // Runtime check to satisfy test framework
        assert_eq!(ARGON2_M_COST, 65536);
        assert_eq!(ARGON2_T_COST, 3);
        assert_eq!(ARGON2_P_COST, 4);
    }
}
