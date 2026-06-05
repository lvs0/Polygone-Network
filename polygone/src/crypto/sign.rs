//! ML-DSA-87 digital signatures — FIPS 204.

use crate::{PolygoneError, Result};
use pqcrypto_mldsa::mldsa87;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// ML-DSA-87 public key.
#[derive(Clone, Debug)]
pub struct SignPublicKey(mldsa87::PublicKey);

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
        let pk = mldsa87::PublicKey::from_bytes(&bytes).map_err(serde::de::Error::custom)?;
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
        Ok(Self(mldsa87::PublicKey::from_bytes(b).map_err(|_| {
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

/// ML-DSA-87 secret key (sensitive).
pub struct SignSecretKey(mldsa87::SecretKey);

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

/// A detached signature (4627 bytes for ML-DSA-87).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature(Vec<u8>);
impl Signature {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Generate a fresh ML-DSA-87 key pair.
pub fn generate_keypair() -> Result<(SignPublicKey, SignSecretKey)> {
    let (pk, sk) = mldsa87::keypair();
    Ok((SignPublicKey(pk), SignSecretKey(sk)))
}

/// Sign arbitrary bytes. Returns a detached signature.
pub fn sign(sk: &SignSecretKey, message: &[u8]) -> Signature {
    // pqcrypto returns a signed message; we detach the signature portion.
    let signed = mldsa87::sign(message, &sk.0);
    let sig_bytes = signed.as_bytes()[..signed.as_bytes().len() - message.len()].to_vec();
    Signature(sig_bytes)
}

/// Verify a detached signature.
pub fn verify(pk: &SignPublicKey, message: &[u8], sig: &Signature) -> Result<()> {
    // Reconstruct signed message for pqcrypto API
    let mut combined = sig.0.clone();
    combined.extend_from_slice(message);
    mldsa87::open(
        &mldsa87::SignedMessage::from_bytes(&combined)
            .map_err(|_| PolygoneError::SignatureInvalid)?,
        &pk.0,
    )
    .map_err(|_| PolygoneError::SignatureInvalid)?;
    Ok(())
}
