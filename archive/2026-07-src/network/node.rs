//! Ephemeral node lifecycle management.

use crate::crypto::error::PolygoneError;
use crate::crypto::kem;
use crate::crypto::sign;
use crate::network::NodeId;
use crate::Result;

/// State of an ephemeral node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    Initializing,
    Active,
    Degraded,
    Disconnected,
    Expired,
}

/// A node in the Polygone network.
#[derive(Debug, Clone)]
pub struct P2pNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Current state
    pub state: NodeState,
    /// KEM public key for key exchange
    pub kem_pk: kem::KemPublicKey,
    /// Signing public key for authentication
    pub sign_pk: sign::SignPublicKey,
    /// Network address (e.g., "/ip4/...")
    pub address: String,
    /// Uptime in seconds
    pub uptime_secs: u64,
}

impl P2pNode {
    /// Create a new node.
    pub fn new(
        id: NodeId,
        kem_pk: kem::KemPublicKey,
        sign_pk: sign::SignPublicKey,
        address: String,
    ) -> Self {
        Self {
            id,
            state: NodeState::Active,
            kem_pk,
            sign_pk,
            address,
            uptime_secs: 0,
        }
    }

    /// Check if the node is active.
    pub fn is_active(&self) -> bool {
        self.state == NodeState::Active
    }

    /// Transition to a new state, validating the transition.
    pub fn transition_to(&mut self, new_state: NodeState) -> Result<()> {
        let from = self.state;
        match (from, new_state) {
            (NodeState::Initializing, NodeState::Active)
            | (NodeState::Active, NodeState::Degraded)
            | (NodeState::Active, NodeState::Disconnected)
            | (NodeState::Active, NodeState::Expired)
            | (NodeState::Degraded, NodeState::Active)
            | (NodeState::Degraded, NodeState::Disconnected)
            | (NodeState::Degraded, NodeState::Expired)
            | (NodeState::Disconnected, NodeState::Active)
            | (NodeState::Disconnected, NodeState::Expired) => {
                self.state = new_state;
                Ok(())
            }
            _ => Err(PolygoneError::InvalidTransition(format!(
                "Cannot transition from {from:?} to {new_state:?}"
            ))),
        }
    }
}