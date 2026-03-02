// Wallet File Management
// Implements wallet.dat file format for persistent key storage

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::SaltString;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::crypto::dilithium::{PublicKey, SecretKey};
use crate::crypto::keys;

#[derive(Debug, thiserror::Error)]
pub enum WalletFileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Encryption error")]
    Encryption,
    #[error("Decryption error")]
    Decryption,
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Wallet file not found")]
    NotFound,
    #[error("Wallet file corrupted")]
    Corrupted,
}

#[derive(Serialize, Deserialize)]
pub struct WalletFile {
    pub version: u32,
    pub created: u64,
    pub address: String,
    pub public_key: Vec<u8>,
    pub encrypted_secret_key: Vec<u8>,
    pub nonce: Vec<u8>,
    pub salt: String,
    pub mnemonic_hint: Option<String>, // First 3 words for verification
}

impl WalletFile {
    /// Creates a new wallet file from a mnemonic and password
    pub fn create_from_mnemonic(
        mnemonic: &str,
        password: &str,
    ) -> Result<Self, WalletFileError> {
        // Derive keypair from mnemonic
        let (pk, sk) = keys::derive_keypair_from_mnemonic(mnemonic);
        let address = keys::encode_address_string(&keys::derive_address(&pk));

        // Generate salt for password hashing
        let salt = SaltString::generate(&mut rand::thread_rng());

        // Derive encryption key from password using Argon2
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| WalletFileError::Encryption)?;

        // Extract key material (first 32 bytes of hash)
        let key_material = password_hash.hash.ok_or(WalletFileError::Encryption)?;
        let key_bytes = key_material.as_bytes();
        if key_bytes.len() < 32 {
            return Err(WalletFileError::Encryption);
        }

        // Create AES-256-GCM cipher
        let cipher = Aes256Gcm::new_from_slice(&key_bytes[..32])
            .map_err(|_| WalletFileError::Encryption)?;

        // Generate random nonce
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt secret key
        let encrypted_secret_key = cipher
            .encrypt(nonce, sk.0.as_ref())
            .map_err(|_| WalletFileError::Encryption)?;

        // Create mnemonic hint (first 3 words)
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        let mnemonic_hint = if words.len() >= 3 {
            Some(format!("{} {} {}...", words[0], words[1], words[2]))
        } else {
            None
        };

        Ok(WalletFile {
            version: 1,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            address,
            public_key: pk.0.to_vec(),
            encrypted_secret_key,
            nonce: nonce_bytes.to_vec(),
            salt: salt.to_string(),
            mnemonic_hint,
        })
    }

    /// Decrypts the secret key using the password
    pub fn decrypt_secret_key(&self, password: &str) -> Result<SecretKey, WalletFileError> {
        // Parse salt
        let salt = SaltString::from_b64(&self.salt).map_err(|_| WalletFileError::Corrupted)?;

        // Derive key from password
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| WalletFileError::InvalidPassword)?;

        // Extract key material
        let key_material = password_hash.hash.ok_or(WalletFileError::Decryption)?;
        let key_bytes = key_material.as_bytes();
        if key_bytes.len() < 32 {
            return Err(WalletFileError::Decryption);
        }

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(&key_bytes[..32])
            .map_err(|_| WalletFileError::Decryption)?;

        // Decrypt
        let nonce = Nonce::from_slice(&self.nonce);
        let decrypted = cipher
            .decrypt(nonce, self.encrypted_secret_key.as_ref())
            .map_err(|_| WalletFileError::InvalidPassword)?;

        // Convert to SecretKey
        if decrypted.len() != crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES {
            return Err(WalletFileError::Corrupted);
        }

        let mut sk_bytes = [0u8; crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES];
        sk_bytes.copy_from_slice(&decrypted);

        Ok(SecretKey(sk_bytes))
    }

    /// Saves the wallet file to disk
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), WalletFileError> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Loads a wallet file from disk
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, WalletFileError> {
        if !path.as_ref().exists() {
            return Err(WalletFileError::NotFound);
        }
        let json = fs::read_to_string(path)?;
        let wallet: WalletFile = serde_json::from_str(&json)?;
        Ok(wallet)
    }

    /// Gets the public key
    pub fn get_public_key(&self) -> Result<PublicKey, WalletFileError> {
        if self.public_key.len() != crate::crypto::dilithium::DILITHIUM3_PUBKEY_BYTES {
            return Err(WalletFileError::Corrupted);
        }
        let mut pk_bytes = [0u8; crate::crypto::dilithium::DILITHIUM3_PUBKEY_BYTES];
        pk_bytes.copy_from_slice(&self.public_key);
        Ok(PublicKey(pk_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_wallet_file_create_and_decrypt() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let password = "test123";

        // Create wallet
        let wallet = WalletFile::create_from_mnemonic(mnemonic, password).unwrap();

        // Verify address
        assert!(wallet.address.starts_with("KOT1"));

        // Decrypt secret key
        let sk = wallet.decrypt_secret_key(password).unwrap();
        assert_eq!(sk.0.len(), crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES);

        // Wrong password should fail
        assert!(wallet.decrypt_secret_key("wrong").is_err());
    }

    #[test]
    fn test_wallet_file_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wallet.dat");

        let mnemonic = "test test test test test test test test test test test test test test test test test test test test test test test test";
        let password = "secure123";

        // Create and save
        let wallet1 = WalletFile::create_from_mnemonic(mnemonic, password).unwrap();
        wallet1.save(&path).unwrap();

        // Load and verify
        let wallet2 = WalletFile::load(&path).unwrap();
        assert_eq!(wallet1.address, wallet2.address);
        assert_eq!(wallet1.public_key, wallet2.public_key);

        // Decrypt with loaded wallet
        let sk = wallet2.decrypt_secret_key(password).unwrap();
        assert_eq!(sk.0.len(), crate::crypto::dilithium::DILITHIUM3_PRIVKEY_BYTES);
    }

    #[test]
    fn test_mnemonic_hint() {
        let mnemonic = "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 word14 word15 word16 word17 word18 word19 word20 word21 word22 word23 word24";
        let wallet = WalletFile::create_from_mnemonic(mnemonic, "pass").unwrap();
        
        assert_eq!(wallet.mnemonic_hint, Some("word1 word2 word3...".to_string()));
    }
}
