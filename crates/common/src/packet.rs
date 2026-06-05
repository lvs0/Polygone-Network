//! Network packet — the wire format between Polygone nodes.
//!
//! Status: stub. Real implementation comes from migrating
//! `polygone::protocol::session` and `polygone::crypto::symmetric`.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use super::identity::NodeId;

/// A single network packet. Bytes are AES-256-GCM ciphertext when
/// `encrypted`, plaintext when not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub from: NodeId,
    pub to:   NodeId,
    pub bytes: Vec<u8>,
    /// Unix epoch milliseconds.
    pub ts_ms: u64,
}

impl Packet {
    pub fn new(from: NodeId, to: NodeId, bytes: Vec<u8>) -> Self {
        Self {
            from, to, bytes,
            ts_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }
}
