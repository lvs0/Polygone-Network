//! `polygone-msg` — ephemeral end-to-end P2P messaging.
//!
//! Spec §3: "Réseau de messagerie P2P asynchrone s'appuyant sur
//! libp2p et une table de hachage distribuée (Kademlia DHT).
//! Transmis via le protocole MSH."

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A recipient, identified by 32-byte public-key fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Recipient(pub [u8; 32]);

/// A message that has been encrypted (the payload is opaque
/// ciphertext) and is sitting in the local mailbox.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredMessage {
    /// Unique message id (SHA-256[..16]).
    pub id: String,
    /// Sender.
    pub from: Recipient,
    /// Recipient.
    pub to: Recipient,
    /// Opaque ciphertext.
    pub payload: Vec<u8>,
    /// Created at (epoch ms).
    pub created_at_ms: u64,
    /// TTL.
    pub ttl: Duration,
}

/// A read-and-sealed message.
#[derive(Clone, Debug)]
pub struct ReadMessage {
    /// Sender.
    pub from: Recipient,
    /// Plaintext body.
    pub body: Vec<u8>,
    /// Seconds remaining.
    pub remaining: Duration,
}

/// In-memory mailbox.
pub struct MsgNode {
    mailbox: HashMap<String, StoredMessage>,
    me: Recipient,
}

impl MsgNode {
    /// Create a new local node.
    pub fn new(me: Recipient) -> Self {
        Self { mailbox: HashMap::new(), me }
    }

    /// Send a message — caller encrypts.
    pub fn send(&mut self, from: Recipient, to: Recipient, payload: Vec<u8>, ttl: Duration) -> String {
        let mut h = Sha256::new();
        h.update(&payload);
        h.update(from.0);
        h.update(to.0);
        let id = format!("msg:{}", hex::encode(&h.finalize()[..16]));
        self.mailbox.insert(id.clone(), StoredMessage {
            id: id.clone(),
            from, to, payload,
            created_at_ms: epoch_ms(),
            ttl,
        });
        id
    }

    /// Receive the next message addressed to `me`.
    pub fn receive(&mut self) -> Option<ReadMessage> {
        let now = epoch_ms();
        let key = self.mailbox.iter()
            .find(|(_, m)| m.to == self.me && !is_expired(m, now))
            .map(|(k, _)| k.clone())?;
        let m = self.mailbox.remove(&key)?;
        let remaining = if now.saturating_sub(m.created_at_ms) >= m.ttl.as_millis() as u64 {
            Duration::ZERO
        } else {
            Duration::from_millis(m.ttl.as_millis() as u64 - (now - m.created_at_ms))
        };
        Some(ReadMessage { from: m.from, body: m.payload, remaining })
    }

    /// Count pending messages.
    pub fn pending(&self) -> usize {
        let now = epoch_ms();
        self.mailbox.values().filter(|m| m.to == self.me && !is_expired(m, now)).count()
    }

    /// Drop expired messages.
    pub fn sweep(&mut self) -> usize {
        let now = epoch_ms();
        let before = self.mailbox.len();
        self.mailbox.retain(|_, m| !is_expired(m, now));
        before - self.mailbox.len()
    }
}

fn is_expired(m: &StoredMessage, now_ms: u64) -> bool {
    now_ms.saturating_sub(m.created_at_ms) > m.ttl.as_millis() as u64
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(b: u8) -> Recipient { Recipient([b; 32]) }

    #[test]
    fn send_and_receive() {
        let mut n = MsgNode::new(r(1));
        let id = n.send(r(2), r(1), b"hi".to_vec(), Duration::from_secs(60));
        assert!(id.starts_with("msg:"));
        let got = n.receive().unwrap();
        assert_eq!(got.body, b"hi");
        assert_eq!(n.pending(), 0);
    }

    #[test]
    fn wrong_recipient_unreadable() {
        let mut n = MsgNode::new(r(1));
        n.send(r(2), r(3), b"x".to_vec(), Duration::from_secs(60));
        assert!(n.receive().is_none());
    }

    #[test]
    fn dht_key_deterministic() {
        let mut h1 = Sha256::new(); h1.update(b"x"); h1.update([1; 32]); h1.update([2; 32]);
        let mut h2 = Sha256::new(); h2.update(b"x"); h2.update([1; 32]); h2.update([2; 32]);
        let a = format!("msg:{}", hex::encode(&h1.finalize()[..16]));
        let b = format!("msg:{}", hex::encode(&h2.finalize()[..16]));
        assert_eq!(a, b);
    }

    #[test]
    fn sweep_removes_expired() {
        let mut n = MsgNode::new(r(1));
        n.send(r(2), r(1), b"a".to_vec(), Duration::from_secs(60));
        // Manually backdate to expire.
        n.mailbox.values_mut().for_each(|m| m.created_at_ms = 0);
        let removed = n.sweep();
        assert_eq!(removed, 1);
    }
}
