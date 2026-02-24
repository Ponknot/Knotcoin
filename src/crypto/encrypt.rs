// Wallet Encryption module: Argon2id + AES-256-GCM
//
// Uses Argon2id for key derivation from a user password,
// and AES-256-GCM for authenticated encryption of the Dilithium3 private key.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use rand::{RngCore, thread_rng};

pub struct EncryptedWallet {
    pub ciphertext: Vec<u8>,
    pub salt: [u8; 16],
    pub nonce: [u8; 12],
}

pub fn encrypt_seed(seed: &[u8], password: &str) -> Result<EncryptedWallet, &'static str> {
    let mut salt_bytes = [0u8; 16];
    thread_rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| "salt encoding failed")?;

    // Derive 32-byte key using Argon2id
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| "password hashing failed")?;
    
    let key_bytes = hash.hash.as_ref().ok_or("hash extraction failed")?.as_bytes();
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, seed)
        .map_err(|_| "encryption failed")?;

    Ok(EncryptedWallet {
        ciphertext,
        salt: salt_bytes,
        nonce: nonce_bytes,
    })
}

pub fn decrypt_seed(wallet: &EncryptedWallet, password: &str) -> Result<Vec<u8>, &'static str> {
    let salt = SaltString::encode_b64(&wallet.salt).map_err(|_| "salt encoding failed")?;
    
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| "password hashing failed")?;
    
    let key_bytes = hash.hash.as_ref().ok_or("hash extraction failed")?.as_bytes();
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&wallet.nonce);

    let plaintext = cipher
        .decrypt(nonce, wallet.ciphertext.as_ref())
        .map_err(|_| "decryption failed (wrong password?)")?;

    Ok(plaintext)
}
