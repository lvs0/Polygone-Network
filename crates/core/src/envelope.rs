//! Cryptography-shaped envelope — a placeholder for the real ML-KEM / Shamir
//! work. The point of v2 is to have a *wire-compatible* envelope that the
//! audit test (tests/integration.rs) can verify replicates "relay sees nothing"
//! at the protocol level. We swap in real crypto at the encryption step later.

use serde::{Deserialize, Serialize};

use crate::error::PolygoneError;
use crate::identity::{NodeId, SessionId};

/// Number of Shamir shares to generate per message.
pub const FRAGMENT_SHARES: usize = 7;
/// Minimum number of shares required to reconstruct.
pub const FRAGMENT_THRESHOLD: usize = 4;

/// Kind of message being carried by an envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    /// A handshake init (peer introduction)
    HandshakeInit,
    /// A handshake ack (peer acknowledgement)
    HandshakeAck,
    /// A data fragment (encrypted + Shamir-shared)
    Fragment,
    /// Acknowledgement of fragment receipt
    Ack,
    /// Dissolve signal — tear down a session
    Dissolve,
}

/// A single Shamir fragment of an encrypted payload.
/// The relay sees these — but never has enough (threshold) to reconstruct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fragment {
    /// Session this fragment belongs to.
    pub session_id: SessionId,
    /// Fragment index (0..FRAGMENT_SHARES).
    pub index: u8,
    /// Threshold required to reconstruct.
    pub threshold: u8,
    /// Total shares produced.
    pub total: u8,
    /// BLAKE3 hash of the full ciphertext (lets receiver verify reassembly).
    pub content_hash: [u8; 32],
    /// Raw fragment bytes (Shamir share).
    pub payload: Vec<u8>,
}

impl Fragment {
    /// Dummy content hash for development — real impl uses blake3.
    pub fn dummy_hash(content: &[u8]) -> [u8; 32] {
        let mut h = [0u8; 32];
        for (i, b) in content.iter().enumerate() {
            h[i % 32] ^= b;
        }
        h
    }
}

/// The wire envelope. This is what flows over the network.
/// The relay only ever sees Envelopes with kind=Fragment; everything else
/// (HandshakeInit/Ack/Dissolve) stays between Alice and Bob via libp2p directly
/// (handled by `polygone-client`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// What kind of payload this is.
    pub kind: EnvelopeKind,
    /// Source peer (random 16-byte ID, no PII).
    pub from: NodeId,
    /// Destination peer.
    pub to: NodeId,
    /// Optional session binding.
    pub session: Option<SessionId>,
    /// Sequence number for this session.
    pub seq: u64,
    /// Unix timestamp seconds (relay never inspects this content-by-content,
    /// but it's useful for client-side ordering).
    pub timestamp: u64,
    /// Payload — encrypted bytes for Fragment kind, raw for HandshakeInit/Ack.
    pub payload: Vec<u8>,
}

impl Envelope {
    /// Create a new envelope with the current unix timestamp.
    pub fn new(kind: EnvelopeKind, from: NodeId, to: NodeId) -> Self {
        Self {
            kind,
            from,
            to,
            session: None,
            seq: 0,
            timestamp: now_secs(),
            payload: Vec::new(),
        }
    }

    /// True if the relay is allowed to handle this envelope kind.
    /// The relay ONLY sees Fragments. Handshake/Dissolve go peer-to-peer.
    pub fn relay_visible(&self) -> bool {
        matches!(self.kind, EnvelopeKind::Fragment)
    }

    /// Create a fragment envelope from a Fragment.
    pub fn from_fragment(from: NodeId, to: NodeId, frag: &Fragment) -> Self {
        let payload = serde_json::to_vec(frag).expect("Fragment serialises");
        let mut e = Self::new(EnvelopeKind::Fragment, from, to);
        e.session = Some(frag.session_id);
        e.payload = payload;
        e
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_roundtrip() {
        let from = NodeId::random();
        let to = NodeId::random();
        let mut e = Envelope::new(EnvelopeKind::HandshakeInit, from.clone(), to.clone());
        e.payload = b"hello".to_vec();
        let json = serde_json::to_string(&e).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.payload, b"hello");
        assert_eq!(back.from, from);
        assert_eq!(back.to, to);
    }

    #[test]
    fn test_relay_only_sees_fragments() {
        assert!(Envelope::new(EnvelopeKind::Fragment, NodeId::random(), NodeId::random()).relay_visible());
        assert!(!Envelope::new(EnvelopeKind::HandshakeInit, NodeId::random(), NodeId::random()).relay_visible());
        assert!(!Envelope::new(EnvelopeKind::Dissolve, NodeId::random(), NodeId::random()).relay_visible());
    }

    #[test]
    fn test_fragment_serde() {
        let to = NodeId::random();
        let frag = Fragment {
            session_id: SessionId::random(),
            index: 2,
            threshold: FRAGMENT_THRESHOLD as u8,
            total: FRAGMENT_SHARES as u8,
            content_hash: Fragment::dummy_hash(b"abc"),
            payload: vec![1, 2, 3, 4],
        };
        let envelope = Envelope::from_fragment(NodeId::random(), to, &frag);
        assert!(envelope.relay_visible());
        let json = serde_json::to_string(&envelope).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session, Some(frag.session_id));
    }
}
