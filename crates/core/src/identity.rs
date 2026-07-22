//! Identity. Just opaque random IDs — no PII, no derivation from real identity.
//!
//! Why? Because the relay sees these IDs all day long, and if they were
//! derivable from anything stable (email, IP, wallet, …) an observer could
//! correlate them. So they're random 16-byte values, generated locally,
//! and held only in memory.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A peer identifier — opaque random bytes, 16 bytes long.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub [u8; 16]);

/// A session identifier — opaque random bytes, 16 bytes long.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId([u8; 16]);

impl NodeId {
    /// Brand-new random NodeId using the OS CSPRNG.
    pub fn random() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] { &self.0 }
}

impl SessionId {
    pub fn random() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 16] { &self.0 }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", hex::encode_short(&self.0))
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode_short(&self.0))
    }
}

impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Session({})", hex::encode_short(&self.0))
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode_short(&self.0))
    }
}

mod hex {
    pub fn encode_short(b: &[u8]) -> String {
        let s = b.iter().map(|x| format!("{:02x}", x)).collect::<String>();
        if s.len() > 8 { format!("{}…", &s[..8]) } else { s }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_node_ids_differ() {
        let a = NodeId::random();
        let b = NodeId::random();
        assert_ne!(a, b);
    }

    #[test]
    fn test_node_id_roundtrip_serde() {
        let id = NodeId::random();
        let json = serde_json::to_string(&id).unwrap();
        let back: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn test_session_id_roundtrip_serde() {
        let id = SessionId::random();
        let json = serde_json::to_string(&id).unwrap();
        let back: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
