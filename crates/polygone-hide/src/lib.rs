//! `polygone-hide` — SOCKS5 anonymisation proxy.
//!
//! Spec §3: "Module de protection de la vie privée transformant
//! l'ensemble du trafic réseau sortant en flux chiffré routé via
//! un réseau d'anonymat décentralisé."

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// An anonymised circuit (guard → middle → exit hops).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Circuit {
    pub id: String,
    pub hops: Vec<SocketAddr>,
    pub built_at_ms: u64,
    pub ttl: Duration,
}

impl Circuit {
    fn is_alive(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.built_at_ms) < self.ttl.as_millis() as u64
    }
}

/// SOCKS5 proxy config.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    pub bind: SocketAddr,
    pub guards: Vec<SocketAddr>,
    pub exits: Vec<SocketAddr>,
    pub rotation: Duration,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9050),
            guards: vec![],
            exits: vec![],
            rotation: Duration::from_secs(600),
        }
    }
}

/// Runtime proxy stats.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ProxyStats {
    pub bytes_relayed: u64,
    pub active_conns: u32,
    pub total_conns: u64,
    pub refused: u64,
}

/// An anonymised SOCKS5 proxy.
pub struct HideProxy {
    cfg: ProxyConfig,
    circuits: HashMap<String, Circuit>,
    stats: ProxyStats,
}

impl HideProxy {
    /// Create a new proxy.
    pub fn new(cfg: ProxyConfig) -> Self {
        Self { cfg, circuits: HashMap::new(), stats: ProxyStats::default() }
    }

    /// Build a new circuit through guard[0] and exit[0].
    pub fn build_circuit(&mut self) -> Option<String> {
        if self.cfg.guards.is_empty() || self.cfg.exits.is_empty() {
            self.stats.refused += 1;
            return None;
        }
        let g = self.cfg.guards[0];
        let e = self.cfg.exits[0];
        let id = format!("circ:{}:{}", g.port(), e.port());
        self.circuits.insert(id.clone(), Circuit {
            id: id.clone(), hops: vec![g, e],
            built_at_ms: epoch_ms(), ttl: self.cfg.rotation,
        });
        Some(id)
    }

    /// Pick an alive circuit, or build one.
    pub fn pick_circuit(&mut self) -> Option<String> {
        let now = epoch_ms();
        if let Some((k, _)) = self.circuits.iter().find(|(_, c)| c.is_alive(now)) {
            return Some(k.clone());
        }
        self.build_circuit()
    }

    /// Record a new connection — returns its conn id.
    pub fn open_conn(&mut self) -> u64 {
        self.stats.total_conns += 1;
        self.stats.active_conns += 1;
        self.stats.total_conns
    }

    /// Record `n` bytes relayed.
    pub fn record_traffic(&mut self, n: u64) { self.stats.bytes_relayed += n; }

    /// Record a closed connection.
    pub fn close_conn(&mut self) { self.stats.active_conns = self.stats.active_conns.saturating_sub(1); }

    /// Drop expired circuits.
    pub fn sweep(&mut self) -> usize {
        let now = epoch_ms();
        let before = self.circuits.len();
        self.circuits.retain(|_, c| c.is_alive(now));
        before - self.circuits.len()
    }

    /// Stats snapshot.
    pub fn stats(&self) -> ProxyStats { self.stats }
    /// Config.
    pub fn config(&self) -> &ProxyConfig { &self.cfg }
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ProxyConfig {
        ProxyConfig {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9051),
            guards: vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 4001)],
            exits: vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5001)],
            rotation: Duration::from_secs(600),
        }
    }

    #[test]
    fn empty_start() {
        let p = HideProxy::new(cfg());
        assert_eq!(p.stats().active_conns, 0);
        assert_eq!(p.stats().bytes_relayed, 0);
    }

    #[test]
    fn build_circuit() {
        let mut p = HideProxy::new(cfg());
        assert!(p.build_circuit().unwrap().starts_with("circ:"));
    }

    #[test]
    fn refused_when_no_exits() {
        let mut c = cfg();
        c.exits.clear();
        let mut p = HideProxy::new(c);
        assert!(p.build_circuit().is_none());
        assert_eq!(p.stats().refused, 1);
    }

    #[test]
    fn pick_reuses_circuit() {
        let mut p = HideProxy::new(cfg());
        let a = p.pick_circuit().unwrap();
        let b = p.pick_circuit().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn sweep_drops_expired() {
        let mut p = HideProxy::new(cfg());
        let id = p.build_circuit().unwrap();
        p.circuits.get_mut(&id).unwrap().built_at_ms = 0;
        assert_eq!(p.sweep(), 1);
    }

    #[test]
    fn conn_counters() {
        let mut p = HideProxy::new(cfg());
        p.open_conn();
        p.open_conn();
        assert_eq!(p.stats().active_conns, 2);
        p.close_conn();
        assert_eq!(p.stats().active_conns, 1);
    }
}
