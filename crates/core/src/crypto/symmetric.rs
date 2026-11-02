//! AES-256-GCM symmetric encryption.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::SharedSecret;
use crate::{PolygoneError, Result};

/// The result of encrypting a payload: ciphertext + nonce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// AES-256-GCM ciphertext (includes the 16-byte auth tag).
    pub ciphertext: Vec<u8>,
    /// 96-bit random nonce. Never reuse with the same key.
    pub nonce: [u8; 12],
}

/// A 256-bit AES-GCM key, zeroised on drop.
#[derive(ZeroizeOnDrop, Zeroize)]
pub struct SessionKey([u8; 32]);

impl Clone for SessionKey {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never leak key material through Debug.
        f.write_str("SessionKey(***REDACTED***)")
    }
}

impl SessionKey {
    /// Wrap raw bytes into a session key.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Derive a session key from a KEM shared secret using the BLAKE3
    /// domain-separated KDF.
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
        Ok(EncryptedPayload {
            ciphertext,
            nonce: nonce.into(),
        })
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
///
/// Format: `[nonce: 12 bytes][ciphertext: N bytes]`
pub fn encrypt(plaintext: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    let payload = key.encrypt(plaintext)?;
    let mut out = Vec::with_capacity(12 + payload.ciphertext.len());
    out.extend_from_slice(&payload.nonce);
    out.extend_from_slice(&payload.ciphertext);
    Ok(out)
}

/// Convenience: decrypt raw bytes produced by [`encrypt`].
pub fn decrypt(data: &[u8], key: &SessionKey) -> Result<Vec<u8>> {
    if data.len() < 12 {
        return Err(PolygoneError::AeadError(
            "ciphertext too short (< 12 byte nonce)".into(),
        ));
    }
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&data[..12]);
    let payload = EncryptedPayload {
        ciphertext: data[12..].to_vec(),
        nonce,
    };
    key.decrypt(&payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_gcm_round_trip() {
        let key = SessionKey::from_bytes([0x42; 32]);
        let plaintext = b"polygone est un reseau ephemeral";

        let encrypted = key.encrypt(plaintext).unwrap();
        assert_ne!(encrypted.ciphertext, plaintext);

        let decrypted = key.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn aes_gcm_tag_mismatch_detected() {
        let key = SessionKey::from_bytes([0x42; 32]);
        let other = SessionKey::from_bytes([0x24; 32]);
        let encrypted = key.encrypt(b"secret").unwrap();
        assert!(other.decrypt(&encrypted).is_err());
    }

    #[test]
    fn aes_gcm_helper_round_trip() {
        let key = SessionKey::from_bytes([0x01; 32]);
        let raw = encrypt(b"hello", &key).unwrap();
        assert_eq!(decrypt(&raw, &key).unwrap(), b"hello");
    }

    #[test]
    fn derive_from_secret_is_stable() {
        let secret = SharedSecret([0x77; 32]);
        let k1 = SessionKey::derive_from_secret(&secret);
        let k2 = SessionKey::derive_from_secret(&secret);
        assert_eq!(k1.0, k2.0);
    }
}
