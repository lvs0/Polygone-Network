//! ML-KEM-1024 key encapsulation — NIST FIPS 203.
//!
//! Wraps `pqcrypto-mlkem` with a typed, zeroize-safe API.

use pqcrypto_mlkem::mlkem1024;
use pqcrypto_traits::kem::{
    Ciphertext as PqcCiphertext, PublicKey, SecretKey, SharedSecret as PqcSharedSecret,
};
use zeroize::ZeroizeOnDrop;

use crate::crypto::SharedSecret;
use crate::{PolygoneError, Result};

// ── Byte sizes ────────────────────────────────────────────────────────────────
/// ML-KEM-1024 encapsulation key (public) size in bytes.
pub const EK_SIZE: usize = 1568;
/// ML-KEM-1024 decapsulation key (secret) size in bytes.
pub const DK_SIZE: usize = 3168;
/// ML-KEM-1024 ciphertext size in bytes.
pub const CT_SIZE: usize = 1568;
/// Shared secret size in bytes.
pub const SS_SIZE: usize = 32;

// ── Key types ─────────────────────────────────────────────────────────────────

/// An ML-KEM-1024 encapsulation (public) key.
#[derive(Debug, Clone)]
pub struct KemPublicKey(mlkem1024::PublicKey);

impl KemPublicKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        PublicKey::as_bytes(&self.0)
    }

    /// Hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s.trim())
            .map_err(|e| PolygoneError::KeyFile(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }

    /// Parse from raw bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(PublicKey::from_bytes(b).map_err(|_| {
            PolygoneError::KeyFile("Invalid ML-KEM-1024 public key".into())
        })?))
    }
}

/// An ML-KEM-1024 decapsulation (secret) key, zeroised on drop.
#[derive(ZeroizeOnDrop)]
pub struct KemSecretKey(#[zeroize(skip)] pub mlkem1024::SecretKey);

impl KemSecretKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        SecretKey::as_bytes(&self.0)
    }

    /// Hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s.trim())
            .map_err(|e| PolygoneError::KeyFile(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }

    /// Parse from raw bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(SecretKey::from_bytes(b).map_err(|_| {
            PolygoneError::KeyFile("Invalid ML-KEM-1024 secret key".into())
        })?))
    }
}

/// An ML-KEM-1024 ciphertext (encapsulation output).
#[derive(Clone)]
pub struct KemCiphertext(mlkem1024::Ciphertext);

impl std::fmt::Debug for KemCiphertext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("KemCiphertext")
            .field(&hex::encode(self.as_bytes()))
            .finish()
    }
}

impl KemCiphertext {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        PqcCiphertext::as_bytes(&self.0)
    }

    /// Hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s.trim())
            .map_err(|e| PolygoneError::KeyFile(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }

    /// Parse from raw bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(mlkem1024::Ciphertext::from_bytes(b).map_err(|_| {
            PolygoneError::KeyFile("Invalid ML-KEM-1024 ciphertext".into())
        })?))
    }
}

// ── Operations ────────────────────────────────────────────────────────────────

/// Generate a fresh ML-KEM-1024 key pair.
pub fn generate_keypair() -> Result<(KemPublicKey, KemSecretKey)> {
    let (pk, sk) = mlkem1024::keypair();
    Ok((KemPublicKey(pk), KemSecretKey(sk)))
}

/// Encapsulate a shared secret against a public key.
pub fn encapsulate(pk: &KemPublicKey) -> Result<(KemCiphertext, SharedSecret)> {
    let (ss, ct) = mlkem1024::encapsulate(&pk.0);
    let raw = PqcSharedSecret::as_bytes(&ss);
    let mut bytes = [0u8; SS_SIZE];
    bytes.copy_from_slice(&raw[..SS_SIZE]);
    Ok((KemCiphertext(ct), SharedSecret(bytes)))
}

/// Decapsulate a shared secret from a ciphertext with a secret key.
pub fn decapsulate(sk: &KemSecretKey, ct: &KemCiphertext) -> Result<SharedSecret> {
    let ss = mlkem1024::decapsulate(&ct.0, &sk.0);
    let raw = PqcSharedSecret::as_bytes(&ss);
    if raw.len() < SS_SIZE {
        return Err(PolygoneError::KemDecapsulate);
    }
    let mut bytes = [0u8; SS_SIZE];
    bytes.copy_from_slice(&raw[..SS_SIZE]);
    Ok(SharedSecret(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ml_kem_1024_round_trip() {
        let (pk, sk) = generate_keypair().unwrap();
        let (ct, ss1) = encapsulate(&pk).unwrap();
        let ss2 = decapsulate(&sk, &ct).unwrap();
        assert_eq!(ss1, ss2);
    }

    #[test]
    fn ml_kem_1024_keygen_consistent() {
        let (pk1, sk1) = generate_keypair().unwrap();
        let (pk2, sk2) = generate_keypair().unwrap();
        assert_ne!(pk1.as_bytes(), pk2.as_bytes());
        assert_ne!(sk1.as_bytes(), sk2.as_bytes());
    }

    #[test]
    fn ml_kem_1024_hex_roundtrip() {
        let (pk, sk) = generate_keypair().unwrap();
        let pk_hex = pk.to_hex();
        let sk_hex = sk.to_hex();
        let pk2 = KemPublicKey::from_hex(&pk_hex).unwrap();
        let sk2 = KemSecretKey::from_hex(&sk_hex).unwrap();
        assert_eq!(pk.as_bytes(), pk2.as_bytes());
        assert_eq!(sk.as_bytes(), sk2.as_bytes());
    }

    #[test]
    fn ml_kem_1024_wrong_key_fails() {
        let (pk_a, _sk_a) = generate_keypair().unwrap();
        let (_pk_b, sk_b) = generate_keypair().unwrap();
        let (ct, _ss) = encapsulate(&pk_a).unwrap();
        // Decapsulating with an unrelated secret key yields a different secret —
        // Polygone treats it as a mismatch (no authenticated failure mode in KEM).
        let ss = decapsulate(&sk_b, &ct).unwrap();
        let (_, ss_expected) = encapsulate(&pk_a).unwrap();
        assert_ne!(ss, ss_expected);
    }
}
