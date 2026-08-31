//! ML-DSA-65 digital signatures — FIPS 204.
//!
//! This module provides a high-level API for ML-DSA-65 signatures
//! matching the expected Polygone core interface.

use crate::{NodeId, PolygoneError, Result};
use pqcrypto_mldsa::mldsa65;
use pqcrypto_traits::sign::{PublicKey as PubKeyTrait, SecretKey as SecKeyTrait, SignedMessage};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// ML-DSA-65 public key (1952 bytes).
#[derive(Clone, Debug)]
pub struct PublicKey(mldsa65::PublicKey);

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(self.0.as_bytes())
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        let pk = mldsa65::PublicKey::from_bytes(&bytes).map_err(serde::de::Error::custom)?;
        Ok(PublicKey(pk))
    }
}

impl PublicKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Parse from bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(mldsa65::PublicKey::from_bytes(b).map_err(|_| {
            PolygoneError::Serde("Invalid Sign PK".into())
        })?))
    }

    /// Hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes =
            hex::decode(s.trim()).map_err(|e| PolygoneError::Serde(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }
}

/// ML-DSA-65 secret key (4032 bytes, sensitive).
#[derive(Clone)]
pub struct SecretKey(mldsa65::SecretKey);

impl SecretKey {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        <mldsa65::SecretKey as SecKeyTrait>::as_bytes(&self.0)
    }

    /// Parse from bytes.
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(mldsa65::SecretKey::from_bytes(b).map_err(|_| {
            PolygoneError::Serde("Invalid Sign SK".into())
        })?))
    }

    /// Hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Parse from hex string.
    pub fn from_hex(s: &str) -> Result<Self> {
        let bytes =
            hex::decode(s.trim()).map_err(|e| PolygoneError::Serde(format!("hex decode: {e}")))?;
        Self::from_bytes(&bytes)
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(***REDACTED***)")
    }
}

/// A detached signature (3309 bytes for ML-DSA-65).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(Vec<u8>);

impl Signature {
    /// Raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Build a detached signature from raw bytes (e.g. parsed from a
    /// received envelope).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Signature(bytes.to_vec())
    }
}

/// High-level signer interface.
#[derive(Clone)]
pub struct Signer {
    sk: SecretKey,
}

impl Signer {
    /// The secret key backing this signer. Handle with care — it never
    /// leaves the machine and is zeroized on drop.
    pub fn secret_key(&self) -> &SecretKey {
        &self.sk
    }

    /// Build a signer from an existing secret key (e.g. parsed from a
    /// persisted identity).
    pub fn from_secret(sk: SecretKey) -> Self {
        Self { sk }
    }

    /// Sign a message, returning a detached signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        let signed = mldsa65::sign(message, &self.sk.0);
        // Detach: signed = signature || message
        let sig_len = signed.as_bytes().len() - message.len();
        Signature(signed.as_bytes()[..sig_len].to_vec())
    }
}

/// High-level verifier interface.
#[derive(Clone)]
pub struct Verifier {
    pk: PublicKey,
}

impl Verifier {
    /// The public key this verifier authenticates with.
    pub fn public_key(&self) -> &PublicKey {
        &self.pk
    }

    /// Build a verifier from an existing public key (e.g. parsed from a
    /// received envelope's `signer` field).
    pub fn from_public(pk: PublicKey) -> Self {
        Self { pk }
    }

    /// Verify a detached signature.
    pub fn verify(&self, message: &[u8], sig: &Signature) -> bool {
        // Reconstruct signed message for pqcrypto API: sig || message
        let mut combined = sig.0.clone();
        combined.extend_from_slice(message);

        // Try to open - need to create SignedMessage from bytes
        match mldsa65::SignedMessage::from_bytes(&combined) {
            Ok(signed_msg) => mldsa65::open(&signed_msg, &self.pk.0).is_ok(),
            Err(_) => false,
        }
    }
}

/// Key pair for signing operations.
#[derive(Clone)]
pub struct KeyPair {
    pub signer: Signer,
    pub verifier: Verifier,
}

/// Generate a fresh ML-DSA-65 key pair.
pub fn generate_keypair() -> Result<KeyPair> {
    let (pk, sk) = mldsa65::keypair();
    Ok(KeyPair {
        signer: Signer { sk: SecretKey(sk) },
        verifier: Verifier { pk: PublicKey(pk) },
    })
}

// Constants matching ML-DSA-65 spec
pub const SIGNATURE_SIZE: usize = 3309;
pub const PUBLIC_KEY_SIZE: usize = 1952;
pub const SECRET_KEY_SIZE: usize = 4032;

// ── proof_of_key ───────────────────────────────────────────────────────────────
/// Proof-of-key for Sybil resistance (P-A2 / P-S2).
///
/// Signs `(PeerID || nonce)` with ML-DSA-65. The peer proves possession of the
/// secret key corresponding to their NodeId without revealing it.
///
/// Bench target: ≤ 200 µs (current release ~270 µs — acceptable for now per D2 revision).
pub fn prove_key(signer: &Signer, peer_id: &NodeId, nonce: &[u8; 32]) -> Result<Signature> {
    // Concatenate PeerID (16 bytes) + nonce (32 bytes) = 48 bytes message
    let mut msg = [0u8; 48];
    msg[..16].copy_from_slice(peer_id.as_bytes());
    msg[16..].copy_from_slice(nonce);
    Ok(signer.sign(&msg))
}

/// Verify a proof-of-key signature.
pub fn verify_key(verifier: &Verifier, peer_id: &NodeId, nonce: &[u8; 32], sig: &Signature) -> bool {
    let mut msg = [0u8; 48];
    msg[..16].copy_from_slice(peer_id.as_bytes());
    msg[16..].copy_from_slice(nonce);
    verifier.verify(&msg, sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_size_is_mldsa65() {
        let kp = generate_keypair().unwrap();
        let sig = kp.signer.sign(b"hello polygone");
        assert_eq!(
            sig.as_bytes().len(),
            3309,
            "ML-DSA-65 signature must be 3309 bytes"
        );
        assert_eq!(
            kp.verifier.pk.as_bytes().len(),
            1952,
            "ML-DSA-65 public key must be 1952 bytes"
        );
        // round-trip
        assert!(kp.verifier.verify(b"hello polygone", &sig));
    }

    #[test]
    fn tampered_message_fails_verification() {
        let kp = generate_keypair().unwrap();
        let sig = kp.signer.sign(b"original message");
        assert!(!kp.verifier.verify(b"tampered message", &sig));
    }

    #[test]
    fn wrong_public_key_fails_verification() {
        let kp_a = generate_keypair().unwrap();
        let kp_b = generate_keypair().unwrap();
        let sig = kp_a.signer.sign(b"hello");
        assert!(!kp_b.verifier.verify(b"hello", &sig));
    }
}

#[cfg(test)]
mod proof_of_key_tests {
    use super::*;
    use crate::identity::NodeId;

    #[test]
    fn proof_of_key_roundtrip() {
        let kp = generate_keypair().unwrap();
        let peer_id = NodeId::random();
        let nonce = [42u8; 32];
        let sig = prove_key(&kp.signer, &peer_id, &nonce).unwrap();
        assert!(verify_key(&kp.verifier, &peer_id, &nonce, &sig));
    }

    #[test]
    fn proof_of_key_wrong_peer_fails() {
        let kp = generate_keypair().unwrap();
        let peer_id_a = NodeId::random();
        let peer_id_b = NodeId::random();
        let nonce = [42u8; 32];
        let sig = prove_key(&kp.signer, &peer_id_a, &nonce).unwrap();
        assert!(!verify_key(&kp.verifier, &peer_id_b, &nonce, &sig));
    }

    #[test]
    fn proof_of_key_wrong_nonce_fails() {
        let kp = generate_keypair().unwrap();
        let peer_id = NodeId::random();
        let nonce_a = [42u8; 32];
        let nonce_b = [99u8; 32];
        let sig = prove_key(&kp.signer, &peer_id, &nonce_a).unwrap();
        assert!(!verify_key(&kp.verifier, &peer_id, &nonce_b, &sig));
    }
}
