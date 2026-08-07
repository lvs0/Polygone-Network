//! Cryptographic primitives for the POLYGONE protocol.
//!
//! Layered design:
//!
//! ```text
//!  ┌──────────────────────────────────────────┐
//!  │  KEM (ML-KEM-1024)  ←→  Key agreement    │
//!  │  DSA (ML-DSA-87)    ←→  Signatures       │
//!  │  AES-256-GCM        ←→  Payload cipher   │
//!  │  Shamir SS          ←→  Fragment secrets  │
//!  │  BLAKE3             ←→  Hashing / VDF    │
//!  └──────────────────────────────────────────┘
//! ```

use zeroize::{Zeroize, ZeroizeOnDrop};

pub mod kem;
pub mod shamir;
pub mod sign;
pub mod symmetric;
pub mod error;
pub mod karma;

// ── KeyPair ──────────────────────────────────────────────────────────────────

/// A combined key-pair: one KEM key-pair (transport) and one DSA key-pair (auth).
///
/// Kept together so they share the same zeroize lifecycle.
pub struct KeyPair {
    /// KEM secret key — used once per session, then destroyed.
    pub kem_sk: kem::KemSecretKey,
    /// KEM public key — shared with peer out-of-band.
    pub kem_pk: kem::KemPublicKey,
    /// Signing secret key — authorises session establishment.
    pub sign_sk: sign::SignSecretKey,
    /// Signing public key — published in the network DHT.
    pub sign_pk: sign::SignPublicKey,
}

impl KeyPair {
    /// Generate a fresh, random key-pair.
    pub fn generate() -> crate::Result<Self> {
        let (kem_pk, kem_sk) = kem::generate_keypair()?;
        let (sign_pk, sign_sk) = sign::generate_keypair()?;
        Ok(Self { kem_sk, kem_pk, sign_sk, sign_pk })
    }

    /// Serialize the entire key-pair to a single byte vector.
    ///
    /// Layout: [KEM_PK][KEM_SK][SIGN_PK][SIGN_SK]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(1568 + 3168 + 2592 + 4896);
        b.extend_from_slice(self.kem_pk.as_bytes());
        b.extend_from_slice(self.kem_sk.as_bytes());
        b.extend_from_slice(self.sign_pk.as_bytes());
        b.extend_from_slice(self.sign_sk.as_bytes());
        b
    }

    /// Parse a key-pair from a byte vector.
    pub fn from_bytes(b: &[u8]) -> crate::Result<Self> {
        if b.len() != (1568 + 3168 + 2592 + 4896) {
            return Err(crate::PolygoneError::Serialization("Invalid KeyPair length".into()));
        }
        let kem_pk = kem::KemPublicKey::from_bytes(&b[0..1568])?;
        let kem_sk = kem::KemSecretKey::from_bytes(&b[1568..4736])?;
        let sign_pk = sign::SignPublicKey::from_bytes(&b[4736..7328])?;
        let sign_sk = sign::SignSecretKey::from_bytes(&b[7328..12224])?;
        Ok(Self { kem_sk, kem_pk, sign_sk, sign_pk })
    }
}

// ── SharedSecret ─────────────────────────────────────────────────────────────

/// 32 bytes of shared secret produced by KEM, zeroised on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
#[derive(PartialEq, Debug)]
pub struct SharedSecret(pub [u8; 32]);

impl SharedSecret {
    /// Derive topology seed and symmetric session key from this shared secret.
    ///
    /// Uses two **distinct** BLAKE3 domain labels so the outputs are
    /// cryptographically independent:
    ///
    /// ```text
    /// topo_seed    = BLAKE3("polygone topology v1"    || shared_secret)  → 32 bytes
    /// session_key  = BLAKE3("polygone session key v1" || shared_secret)  → 32 bytes
    /// ```
    ///
    /// `topo_seed` is passed to `Topology::derive` — it never touches the
    /// symmetric cipher. `session_key` is passed to `SessionKey::from_bytes`
    /// — it never touches topology derivation. The two are domain-separated
    /// and independent even though they share the same KEM output.
    pub fn derive(&self) -> ([u8; 32], [u8; 32]) {
        let topo_seed    = blake3::derive_key("polygone topology v1",    &self.0);
        let session_key  = blake3::derive_key("polygone session key v1", &self.0);
        (topo_seed, session_key)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_generation_is_deterministically_fresh() {
        let kp1 = KeyPair::generate().unwrap();
        let kp2 = KeyPair::generate().unwrap();
        // Public keys must differ (overwhelming probability)
        assert_ne!(kp1.kem_pk.as_bytes(), kp2.kem_pk.as_bytes());
    }

    #[test]
    fn shared_secret_derivation_is_deterministic() {
        let secret = SharedSecret([0xAB; 32]);
        let (t1, k1) = secret.derive();
        let (t2, k2) = secret.derive();
        assert_eq!(t1, t2);
        assert_eq!(k1, k2);
    }

    #[test]
    fn topology_and_session_key_are_distinct() {
        let secret = SharedSecret([0xCD; 32]);
        let (topo, key) = secret.derive();
        // The two derived values must differ
        assert_ne!(&topo[..], &key[..topo.len()]);
    }

    #[test]
    fn keypair_serialization_roundtrip() {
        let kp1 = KeyPair::generate().unwrap();
        let bytes = kp1.to_bytes();
        let kp2 = KeyPair::from_bytes(&bytes).unwrap();
        
        assert_eq!(kp1.kem_pk.as_bytes(), kp2.kem_pk.as_bytes());
        assert_eq!(kp1.sign_pk.as_bytes(), kp2.sign_pk.as_bytes());
    }
}
