//! Protocol session management for Polygone.
//!
//! Handles session lifecycle, key derivation, and secure channel creation.

use crate::crypto::SharedSecret;
use crate::network::NodeId;

/// A secure session between two peers.
#[derive(Debug, Clone)]
pub struct Session {
    /// Local peer ID
    pub local_id: NodeId,
    /// Remote peer ID
    pub remote_id: NodeId,
    /// Session key material
    pub session_key: [u8; 32],
    /// Topology seed for fragment routing
    pub topology_seed: [u8; 32],
    /// Session expiry timestamp (seconds since epoch)
    pub expires_at: u64,
}

impl Session {
    /// Create a new session from a shared secret and peer IDs.
    pub fn new(local_id: NodeId, remote_id: NodeId, secret: &SharedSecret) -> Self {
        let (topology_seed, session_key) = secret.derive();
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 3600; // 1 hour default TTL

        Self {
            local_id,
            remote_id,
            session_key,
            topology_seed,
            expires_at,
        }
    }

    /// Check if this session is still valid.
    pub fn is_valid(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now < self.expires_at
    }

    /// Rotate the session key, extending the TTL.
    pub fn rotate(&mut self, secret: &SharedSecret) {
        let (topology_seed, session_key) = secret.derive();
        self.topology_seed = topology_seed;
        self.session_key = session_key;
        self.expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 3600;
    }
}

/// Session manager for tracking multiple concurrent sessions.
#[derive(Debug)]
pub struct SessionManager {
    sessions: Vec<Session>,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    /// Register a new session.
    pub fn register(&mut self, session: Session) {
        // Clean expired sessions first
        self.sessions.retain(|s| s.is_valid());
        self.sessions.push(session);
    }

    /// Find a session by remote peer ID.
    pub fn find_by_peer(&self, peer_id: &NodeId) -> Option<&Session> {
        self.sessions.iter().find(|s| &s.remote_id == peer_id && s.is_valid())
    }

    /// Find a session by remote peer ID (mutable).
    pub fn find_by_peer_mut(&mut self, peer_id: &NodeId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| &s.remote_id == peer_id && s.is_valid())
    }

    /// Remove a session.
    pub fn remove(&mut self, peer_id: &NodeId) {
        self.sessions.retain(|s| &s.remote_id != peer_id);
    }

    /// Number of active sessions.
    pub fn active_count(&self) -> usize {
        self.sessions.iter().filter(|s| s.is_valid()).count()
    }

    /// Clean all expired sessions.
    pub fn clean_expired(&mut self) {
        self.sessions.retain(|s| s.is_valid());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_id(value: u8) -> NodeId {
        [value; 32]
    }

    #[test]
    fn session_creation_and_validation() {
        let alice = test_node_id(0xAA);
        let bob = test_node_id(0xBB);
        let secret = SharedSecret([0x42; 32]);

        let session = Session::new(alice, bob, &secret);
        assert_eq!(session.local_id, alice);
        assert_eq!(session.remote_id, bob);
        assert!(session.is_valid());
        assert!(session.session_key != [0u8; 32]);
        assert!(session.topology_seed != [0u8; 32]);
    }

    #[test]
    fn session_manager_workflow() {
        let mut manager = SessionManager::new();
        assert_eq!(manager.active_count(), 0);

        let alice = test_node_id(0xAA);
        let bob = test_node_id(0xBB);
        let secret = SharedSecret([0x42; 32]);
        let session = Session::new(alice, bob, &secret);
        manager.register(session);

        assert_eq!(manager.active_count(), 1);

        let found = manager.find_by_peer(&bob);
        assert!(found.is_some());
        assert_eq!(found.unwrap().local_id, alice);

        manager.remove(&bob);
        assert_eq!(manager.active_count(), 0);
    }
}