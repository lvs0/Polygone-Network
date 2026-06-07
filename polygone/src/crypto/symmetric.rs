//! AES-256-GCM symmetric encryption.

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::crypto::SharedSecret;
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

    /// Derive a session key from a shared secret using BLAKE3 domain-separated KDF.
    pub fn derive_from_secret(secret: &SharedSecret) -> Self {
        let (_, session_key_bytes) = secret.derive();
        Self(session_key_bytes)
    }

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

/// Convenience: encrypt plaintext with a session key, return raw bytes.
/// Format: [nonce: 12 bytes][ciphertext: N bytes]
pub fn encrypt(plaintext: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    let payload = key.encrypt(plaintext)?;
    let mut out = Vec::with_capacity(12 + payload.ciphertext.len());
    out.extend_from_slice(&payload.nonce);
    out.extend_from_slice(&payload.ciphertext);
    Ok(out)
}

/// Convenience: decrypt raw bytes with a session key, return plaintext.
/// Expects format: [nonce: 12 bytes][ciphertext: N bytes]
pub fn decrypt(data: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(PolygoneError::Decrypt("data too short".into()));
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&data[..12]);
    let ciphertext = data[12..].to_vec();
    let payload = EncryptedPayload { ciphertext, nonce };
    key.decrypt(&payload)
}
