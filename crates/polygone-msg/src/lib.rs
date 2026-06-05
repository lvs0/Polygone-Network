//! `polygone-msg` — ephemeral end-to-end P2P messaging.
//!
//! Spec §3: "Réseau de messagerie P2P asynchrone s'appuyant sur
//! libp2p et une table de hachage distribuée (Kademlia DHT).
//! Transmission des messages éclatés en fragments de façon
//! transparente."
//!
//! Status: stub. Will be implemented in Phase 3 of the spec roadmap.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder service trait. Real implementation will be a
/// libp2p-based P2P node using Kademlia for peer discovery and
/// mDNS for local network bootstrap.
pub trait Messenger {
    /// Send a message to a peer identified by its `NodeId`.
    /// Returns the message id (so the caller can poll for delivery).
    fn send(&self, to: &str, body: &[u8]) -> String;
    /// Receive the next pending message. Blocks if none.
    fn recv(&self) -> Option<Received>;
}

#[derive(Debug, Clone)]
pub struct Received {
    pub from: String,
    pub body: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_polygone_msg() {
        assert!(VERSION.starts_with("1."));
    }
}
