//! Ephemeral network topology and node lifecycle.
//!
//! This module provides the P2P networking infrastructure for Polygone,
//! including the unified P2P layer (`p2p`), ephemeral node management,
//! and topology derivation.

pub mod discovery;
pub mod node;
pub mod topology;

pub mod p2p;

pub use self::node::P2pNode;
pub use self::discovery::*;
pub use self::topology::{Topology, TopologyParams};

/// Represents a node's unique identifier in the P2P network.
pub type NodeId = [u8; 32];

/// Configuration for a P2P node.
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Listen address (e.g. "/ip4/0.0.0.0/tcp/4001")
    pub listen_addr: String,
    /// Bootstrap nodes for DHT
    pub bootstrap_nodes: Vec<String>,
    /// Enable mDNS local discovery
    pub mdns_enabled: bool,
    /// Maximum number of peers
    pub max_peers: usize,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/4001".into(),
            bootstrap_nodes: vec![],
            mdns_enabled: true,
            max_peers: 50,
        }
    }
}

/// Events emitted by the network layer.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    PeerConnected(NodeId),
    PeerDisconnected(NodeId),
    MessageReceived(NodeId, Vec<u8>),
    TopologyChange(Vec<NodeId>),
}

/// Stub request type.
#[derive(Debug, Clone)]
pub struct PolygoneRequest;

/// Stub response type.
#[derive(Debug, Clone)]
pub struct PolygoneResponse;

/// Stub gossip message.
#[derive(Debug, Clone)]
pub struct GossipMessage;

/// Stub capability.
#[derive(Debug, Clone)]
pub struct Capability;