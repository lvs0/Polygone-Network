//! Node Discovery Module for Polygone P2P Network
//!
//! Provides peer discovery logic using Kademlia DHT and mDNS

use libp2p::{
    kad::{QueryResult, GetRecordOk, Record},
    PeerId,
};
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// DiscoveryResult contains discovered peer information
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// Peer ID
    pub peer_id: PeerId,
    /// Observed addresses
    pub addresses: Vec<libp2p::Multiaddr>,
    /// Whether peer supports Drive
    pub supports_drive: bool,
    /// Whether peer supports Petals
    pub supports_petals: bool,
    /// Whether peer is a relay
    pub is_relay: bool,
}

/// DiscoveryService handles peer discovery logic
pub struct DiscoveryService {
    /// Known peer IDs
    peers: HashSet<PeerId>,
    /// Bootstrap nodes
    bootstrap_nodes: Vec<PeerId>,
}

impl DiscoveryService {
    /// Create new DiscoveryService
    pub fn new() -> Self {
        Self {
            peers: HashSet::new(),
            bootstrap_nodes: Vec::new(),
        }
    }

    /// Add a bootstrap node
    pub fn add_bootstrap(&mut self, peer_id: PeerId) {
        self.bootstrap_nodes.push(peer_id);
        info!("Added bootstrap node: {}", peer_id);
    }

    /// Record a discovered peer
    pub fn record_peer(&mut self, peer_id: PeerId) {
        if self.peers.insert(peer_id) {
            debug!("Discovered new peer: {}", peer_id);
        }
    }

    /// Get all known peers
    pub fn get_peers(&self) -> Vec<PeerId> {
        self.peers.iter().cloned().collect()
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check if we have any peers
    pub fn has_peers(&self) -> bool {
        !self.peers.is_empty()
    }

    /// Handle DHT query result
    pub fn handle_dht_result(&mut self, result: &QueryResult) {
        match result {
            _ => {
                debug!("DHT result: {:?}", result);
            }
        }
    }

    /// Get bootstrap node count
    pub fn bootstrap_count(&self) -> usize {
        self.bootstrap_nodes.len()
    }

    /// Clear all known peers
    pub fn clear_peers(&mut self) {
        self.peers.clear();
        debug!("Cleared all discovered peers");
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_service() {
        let mut service = DiscoveryService::new();
        assert_eq!(service.peer_count(), 0);
        assert!(!service.has_peers());
    }
}
