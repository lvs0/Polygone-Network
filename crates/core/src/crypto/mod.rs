//! Post-quantum cryptographic primitives for the POLYGONE protocol.
//!
//! Layered design:
//!
//! ```text
//!  ┌──────────────────────────────────────────┐
//!  │  KEM (ML-KEM-1024)  ←→  Key agreement    │
//!  │  DSA (ML-DSA-65)    ←→  Signatures       │
//!  │  AES-256-GCM        ←→  Payload cipher   │
//!  │  Shamir SS          ←→  Fragment secrets  │
//!  │  BLAKE3             ←→  Hashing / KDF    │
//!  └──────────────────────────────────────────┘
//! ```

pub mod kem;
pub mod shamir;
pub mod symmetric;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// 32 bytes of shared secret produced by ML-KEM-1024, zeroised on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq, Debug)]
pub struct SharedSecret(pub [u8; 32]);

impl SharedSecret {
    /// Derive the topology seed and the symmetric session key from this
    /// shared secret.
    ///
    /// Uses two **distinct** BLAKE3 domain labels so the outputs are
    /// cryptographically independent:
    ///
    /// ```text
    /// topo_seed    = BLAKE3("polygone topology v1"    || shared_secret)  → 32 bytes
    /// session_key  = BLAKE3("polygone session key v1" || shared_secret)  → 32 bytes
    /// ```
    pub fn derive(&self) -> ([u8; 32], [u8; 32]) {
        let topo_seed = blake3::derive_key("polygone topology v1", &self.0);
        let session_key = blake3::derive_key("polygone session key v1", &self.0);
        (topo_seed, session_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_secret_derivation_is_deterministic() {
        let secret = SharedSecret([0xAB; 32]);
        let (t1, k1) = secret.derive();
        let (t2, k2) = secret.derive();
        assert_eq!(t1, t2);
        assert_eq!(k1, k2);
    }

    #[test]
    fn derive_outputs_are_domain_separated() {
        let secret = SharedSecret([0xAB; 32]);
        let (topo, key) = secret.derive();
        // Topology seed and session key must never coincide.
        assert_ne!(topo, key);
    }

    #[test]
    fn derive_outputs_depend_on_input() {
        let s1 = SharedSecret([0x00; 32]);
        let s2 = SharedSecret([0x01; 32]);
        let (t1, _) = s1.derive();
        let (t2, _) = s2.derive();
        assert_ne!(t1, t2);
    }
}
