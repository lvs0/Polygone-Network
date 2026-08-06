//! ML-DSA-65 digital signatures — FIPS 204.

use crate::{PolygoneError, Result};
use pqcrypto_mldsa::mldsa65;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// ML-DSA-65 public key.
#[derive(Clone, Debug)]
pub struct SignPublicKey(mldsa65::PublicKey);

impl Serialize for SignPublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

impl<'de> Deserialize<'de> for SignPublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        let pk = mldsa65::PublicKey::from_bytes(&bytes).map_err(serde::de::Error::custom)?;
        Ok(SignPublicKey(pk))
    }
}

impl SignPublicKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Parse from bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(mldsa65::PublicKey::from_bytes(b).map_err(|_| {
            PolygoneError::Serialization("Invalid Sign PK".into())
        })?))
    }

    /// Hex string.
    pub fn to_hex(&self) -> String { hex::encode(self.as_bytes()) }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s.trim())
            .map_err(|e| PolygoneError::KeyFile(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }
}

/// ML-DSA-65 secret key (sensitive).
pub struct SignSecretKey(mldsa65::SecretKey);

impl SignSecretKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        SecretKey::as_bytes(&self.0)
    }

    /// Parse from bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(SecretKey::from_bytes(b).map_err(|_| {
            PolygoneError::Serialization("Invalid Sign SK".into())
        })?))
    }

    /// Hex string.
    pub fn to_hex(&self) -> String { hex::encode(self.as_bytes()) }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes = hex::decode(s.trim())
            .map_err(|e| PolygoneError::KeyFile(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }
}

/// A detached signature (3309 bytes for ML-DSA-65).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature(Vec<u8>);
impl Signature {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Generate a fresh ML-DSA-65 key pair.
pub fn generate_keypair() -> Result<(SignPublicKey, SignSecretKey)> {
    let (pk, sk) = mldsa65::keypair();
    Ok((SignPublicKey(pk), SignSecretKey(sk)))
}

/// Sign arbitrary bytes. Returns a detached signature.
pub fn sign(sk: &SignSecretKey, message: &[u8]) -> Signature {
    // pqcrypto returns a signed message; we detach the signature portion.
    let signed = mldsa65::sign(message, &sk.0);
    let sig_bytes = signed.as_bytes()[..signed.as_bytes().len() - message.len()].to_vec();
    Signature(sig_bytes)
}

/// Verify a detached signature.
pub fn verify(pk: &SignPublicKey, message: &[u8], sig: &Signature) -> Result<()> {
    // Reconstruct signed message for pqcrypto API
    let mut combined = sig.0.clone();
    combined.extend_from_slice(message);
    mldsa65::open(
        &mldsa65::SignedMessage::from_bytes(&combined)
            .map_err(|_| PolygoneError::SignatureInvalid)?,
        &pk.0,
    )
    .map_err(|_| PolygoneError::SignatureInvalid)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ML-DSA-65 sweet spot — enforce the size invariant explicitly
    /// (P6 roadmap 2026-06-29 + Conseil des Sages Galois).
    /// pk=1952 B · sk=4032 B · sig=3309 B.
    #[test]
    fn signature_size_is_mldsa65() {
        let (pk, sk) = generate_keypair().unwrap();
        let sig = sign(&sk, b"hello polygone");
        assert_eq!(
            sig.as_bytes().len(),
            3309,
            "ML-DSA-65 signature must be 3309 bytes"
        );
        assert_eq!(
            pk.as_bytes().len(),
            1952,
            "ML-DSA-65 public key must be 1952 bytes"
        );
        // round-trip
        verify(&pk, b"hello polygone", &sig).unwrap();
    }

    #[test]
    fn tampered_message_fails_verification() {
        let (pk, sk) = generate_keypair().unwrap();
        let sig = sign(&sk, b"original message");
        assert!(verify(&pk, b"tampered message", &sig).is_err());
    }

    #[test]
    fn wrong_public_key_fails_verification() {
        let (_pk_a, sk_a) = generate_keypair().unwrap();
        let (pk_b, _sk_b) = generate_keypair().unwrap();
        let sig = sign(&sk_a, b"hello");
        assert!(verify(&pk_b, b"hello", &sig).is_err());
    }
}
