//! `polygone-mesh` — local P2P mesh (mDNS over Wi-Fi).
//!
//! Spec §3: "Module de mise en réseau local pair à pair
//! (Wi-Fi/Bluetooth) pour la découverte automatique d'autres
//! instances de Polygone à proximité."
//!
//! Spec §3: "Répartiteur de charge intelligent, fragmentation
//! des tâches sur le réseau maillé."

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A peer on the local mesh.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    /// Stable id (BLAKE3 of the node's public key).
    pub id: String,
    /// Human-readable node name.
    pub name: String,
    /// Transport address (host:port).
    pub address: String,
    /// Discovered at.
    pub discovered_at_ms: u64,
    /// Last seen (heartbeat).
    pub last_seen_ms: u64,
    /// Approximate RTT in ms.
    pub rtt_ms: u32,
    /// CPU load 0..=100.
    pub cpu_load: u8,
    /// Free RAM in MiB.
    pub free_ram_mib: u32,
    /// Currently running tasks.
    pub running_tasks: u32,
    /// Total tasks completed.
    pub tasks_completed: u64,
}

impl Peer {
    /// True if the peer has been seen within the freshness window.
    pub fn is_fresh(&self, now_ms: u64, freshness: Duration) -> bool {
        now_ms.saturating_sub(self.last_seen_ms) < freshness.as_millis() as u64
    }

    /// Load-balancing score — lower is better.
    pub fn load_score(&self) -> u32 {
        let cpu = (self.cpu_load as u32) * 10;
        let ram_penalty = if self.free_ram_mib < 256 { 500 } else { 0 };
        let task_penalty = self.running_tasks * 50;
        cpu + ram_penalty + task_penalty
    }
}

/// A task to be dispatched on the mesh.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshTask {
    pub id: String,
    pub kind: String,
    pub cpu_cost: u8,
    pub ram_mib: u32,
    pub created_at_ms: u64,
}

impl MeshTask {
    /// Pick the best peer for this task, or `None`.
    pub fn best_peer<'a>(&self, peers: &'a [Peer]) -> Option<&'a Peer> {
        peers.iter()
            .filter(|p| (p.cpu_load as u16 + self.cpu_cost as u16) <= 100)
            .filter(|p| p.free_ram_mib >= self.ram_mib)
            .min_by_key(|p| p.load_score())
    }
}

/// Local mesh node.
pub struct MeshNode {
    peers: HashMap<String, Peer>,
    tasks: HashMap<String, MeshTask>,
    freshness: Duration,
}

impl MeshNode {
    /// Create a new node with a 30s freshness window.
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            tasks: HashMap::new(),
            freshness: Duration::from_secs(30),
        }
    }

    /// Announce or refresh a peer.
    pub fn announce(&mut self, mut peer: Peer) {
        let now = epoch_ms();
        peer.last_seen_ms = now;
        if !self.peers.contains_key(&peer.id) {
            peer.discovered_at_ms = now;
        }
        self.peers.insert(peer.id.clone(), peer);
    }

    /// Submit a task — returns the chosen peer id or `None`.
    pub fn submit(&mut self, task: MeshTask) -> Option<String> {
        let snapshot: Vec<Peer> = self.peers.values().cloned().collect();
        let chosen = task.best_peer(&snapshot)?.id.clone();
        if let Some(p) = self.peers.get_mut(&chosen) {
            p.running_tasks += 1;
        }
        self.tasks.insert(task.id.clone(), task);
        Some(chosen)
    }

    /// Mark a task as completed.
    pub fn complete(&mut self, task_id: &str) -> bool {
        self.tasks.remove(task_id).is_some()
    }

    /// Drop stale peers.
    pub fn sweep(&mut self) -> usize {
        let now = epoch_ms();
        let before = self.peers.len();
        self.peers.retain(|_, p| p.is_fresh(now, self.freshness));
        before - self.peers.len()
    }

    /// Sorted peer list (best first).
    pub fn peers(&self) -> Vec<&Peer> {
        let mut v: Vec<&Peer> = self.peers.values().collect();
        v.sort_by_key(|p| p.load_score());
        v
    }

    /// Number of pending tasks.
    pub fn pending_tasks(&self) -> usize { self.tasks.len() }
}

impl Default for MeshNode {
    fn default() -> Self { Self::new() }
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(id: &str, cpu: u8, ram: u32, tasks: u32) -> Peer {
        Peer {
            id: id.into(),
            name: format!("node-{id}"),
            address: "127.0.0.1:4001".into(),
            discovered_at_ms: epoch_ms(),
            last_seen_ms: epoch_ms(),
            rtt_ms: 5,
            cpu_load: cpu,
            free_ram_mib: ram,
            running_tasks: tasks,
            tasks_completed: 0,
        }
    }
    fn task(cpu: u8, ram: u32) -> MeshTask {
        MeshTask {
            id: "t1".into(), kind: "test".into(),
            cpu_cost: cpu, ram_mib: ram, created_at_ms: epoch_ms(),
        }
    }

    #[test]
    fn announce_and_count() {
        let mut m = MeshNode::new();
        m.announce(peer("a", 10, 1024, 0));
        m.announce(peer("b", 20, 2048, 1));
        assert_eq!(m.peers().len(), 2);
    }

    #[test]
    fn best_peer_picks_lowest_load() {
        let mut m = MeshNode::new();
        m.announce(peer("busy", 90, 1024, 10));
        m.announce(peer("idle", 5, 4096, 0));
        assert_eq!(m.submit(task(10, 256)), Some("idle".into()));
    }

    #[test]
    fn too_heavy_task_filtered() {
        let mut m = MeshNode::new();
        m.announce(peer("a", 80, 1024, 0));
        assert!(m.submit(task(50, 256)).is_none());
    }

    #[test]
    fn sweep_drops_stale() {
        let mut m = MeshNode::new();
        m.announce(peer("a", 0, 1024, 0));
        m.peers.get_mut("a").unwrap().last_seen_ms = 0;
        assert_eq!(m.sweep(), 1);
        assert!(m.peers().is_empty());
    }

    #[test]
    fn load_score_punishes_cpu_and_ram() {
        let busy = peer("b", 80, 64, 0);
        let idle = peer("i", 5, 4096, 0);
        assert!(busy.load_score() > idle.load_score());
    }
}