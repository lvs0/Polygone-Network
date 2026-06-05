//! AES-256-GCM symmetric encryption.

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::{PolygoneError, Result};

/// The result of encrypting a payload: ciphertext + nonce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// AES-256-GCM ciphertext (includes 16-byte auth tag).
    pub ciphertext: Vec<u8>,
    /// 96-bit random nonce. Never reuse with the same key.
    pub nonce: [u8; 12],
}

/// A 256-bit AES-GCM key, zeroised on drop.
#[derive(ZeroizeOnDrop, Zeroize)]
pub struct SessionKey([u8; 32]);

impl SessionKey {
    /// Wrap raw bytes into a session key.
    pub fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }

    /// Encrypt `plaintext` and return the ciphertext + nonce.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedPayload> {
        let key = Key::<Aes256Gcm>::from_slice(&self.0);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| PolygoneError::AeadError(e.to_string()))?;
        Ok(EncryptedPayload { ciphertext, nonce: nonce.into() })
    }

    /// Decrypt a previously encrypted payload.
    pub fn decrypt(&self, payload: &EncryptedPayload) -> Result<Vec<u8>> {
        let key = Key::<Aes256Gcm>::from_slice(&self.0);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&payload.nonce);
        cipher
            .decrypt(nonce, payload.ciphertext.as_ref())
            .map_err(|_| PolygoneError::AeadError("decryption failed — tag mismatch".into()))
    }
}
