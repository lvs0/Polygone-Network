//! Deterministic ephemeral topology derivation.
//!
//! Both Alice and Bob independently derive the *same* topology from
//! the shared secret — no extra communication required.
//! The topology defines which 7 nodes exist and how fragments are assigned.

use serde::{Deserialize, Serialize};

use super::NodeId;
use crate::{PolygoneError, Result};

// ── Parameters ────────────────────────────────────────────────────────────────

/// Parameters for the ephemeral network topology.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopologyParams {
    /// Total number of ephemeral nodes. Default: 7.
    pub node_count: u8,
    /// Minimum fragments to reconstruct. Default: 4.
    pub threshold: u8,
}

impl Default for TopologyParams {
    fn default() -> Self {
        Self { node_count: 7, threshold: 4 }
    }
}

impl TopologyParams {
    pub fn validate(&self) -> Result<()> {
        if self.threshold < 2 {
            return Err(PolygoneError::TopologyDerivation(
                "threshold must be ≥ 2".into()
            ));
        }
        if self.node_count < self.threshold {
            return Err(PolygoneError::TopologyDerivation(
                "node_count must be ≥ threshold".into()
            ));
        }
        Ok(())
    }
}

// ── Topology ──────────────────────────────────────────────────────────────────

/// The ephemeral network topology for a single session.
///
/// Deterministically derived from the BLAKE3-expanded topology seed.
/// Both peers compute identical topologies independently.
#[derive(Clone, Debug)]
pub struct Topology {
    pub params: TopologyParams,
    /// Ordered list of ephemeral node IDs.
    pub nodes: Vec<NodeId>,
    /// Map: fragment_index → node_index in `nodes`.
    pub fragment_assignment: Vec<(u8, usize)>,
}

impl Topology {
    /// Derive a topology from `topo_seed` (32 bytes from BLAKE3 key derivation).
    ///
    /// Algorithm:
    /// 1. Expand seed via BLAKE3 XOF to get node IDs (8 bytes each).
    /// 2. Assign fragment i → node i % node_count.
    ///
    /// Both parties, starting from identical seeds, compute identical topologies.
    pub fn derive(topo_seed: &[u8; 32], params: TopologyParams) -> Result<Self> {
        params.validate()?;

        let n = params.node_count as usize;

        // Expand the topology seed using BLAKE3 XOF.
        // We need n * 8 bytes to generate all node IDs.
        let mut output_reader = blake3::Hasher::new_derive_key("polygone-topo-nodes-v1")
            .update(topo_seed)
            .finalize_xof();

        let mut nodes = Vec::with_capacity(n);
        for _ in 0..n {
            let mut id_bytes = [0u8; 32];
            output_reader.fill(&mut id_bytes);
            nodes.push(id_bytes);
        }

        // Simple round-robin fragment assignment:
        // fragment 0 → node 0, fragment 1 → node 1, ..., fragment n-1 → node n-1
        // This is deterministic and distributes load evenly.
        let fragment_assignment: Vec<(u8, usize)> = (0..n)
            .map(|i| (i as u8, i % n))
            .collect();

        Ok(Self { params, nodes, fragment_assignment })
    }

    /// Human-readable description of the topology.
    pub fn describe(&self) -> String {
        format!(
            "{} nodes, threshold {}/{} — {} fragments required to reconstruct",
            self.nodes.len(),
            self.params.threshold,
            self.params.node_count,
            self.params.threshold,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_is_deterministic() {
        let seed = [0xABu8; 32];
        let t1 = Topology::derive(&seed, TopologyParams::default()).unwrap();
        let t2 = Topology::derive(&seed, TopologyParams::default()).unwrap();

        for (a, b) in t1.nodes.iter().zip(t2.nodes.iter()) {
            assert_eq!(a[0], b[0], "Topology must be deterministic");
        }
    }

    #[test]
    fn different_seeds_produce_different_topologies() {
        let seed1 = [0x11u8; 32];
        let seed2 = [0x22u8; 32];
        let t1 = Topology::derive(&seed1, TopologyParams::default()).unwrap();
        let t2 = Topology::derive(&seed2, TopologyParams::default()).unwrap();
        assert_ne!(t1.nodes[0][0], t2.nodes[0][0]);
    }

    #[test]
    fn topology_node_count() {
        let seed = [0u8; 32];
        let t = Topology::derive(&seed, TopologyParams::default()).unwrap();
        assert_eq!(t.nodes.len(), 7);
        assert_eq!(t.fragment_assignment.len(), 7);
    }
}
