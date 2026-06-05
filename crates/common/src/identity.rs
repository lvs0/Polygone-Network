//! Identity types — `NodeId`, `SessionKey`.
//!
//! Status: stub. Will be filled in Phase 1 step 2 by migrating the
//! existing `polygone::crypto::kem` and `polygone::network::node` types.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// 32-byte cryptographically-derived node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 32]);

impl NodeId {
    pub fn random() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }
    pub fn from_bytes(b: [u8; 32]) -> Self { Self(b) }
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

/// Per-session symmetric key. Wraps a 32-byte AES-256 key.
#[derive(Debug, Clone)]
pub struct SessionKey([u8; 32]);

impl SessionKey {
    pub fn from_bytes(b: [u8; 32]) -> Self { Self(b) }
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}
