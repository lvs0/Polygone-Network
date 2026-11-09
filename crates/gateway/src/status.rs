//! État réseau **mesuré** — jamais décoré.
//!
//! Deux chiffres suffisent à mentir sans le vouloir :
//! - `active` : un nœud connu n'est pas un nœud vivant, on ne compte que ceux
//!   vus dans les cinq dernières minutes ;
//! - `latency_ms_median` : une médiane sans mesure n'existe pas, donc elle
//!   vaut `null` dans le JSON (`Option<u64>`) au lieu de `0`, qui se lirait
//!   comme « latence nulle ».

use serde::{Deserialize, Serialize};

use crate::{Gateway, now_secs};

/// Fenêtre pendant laquelle un nœud compte comme actif : 5 minutes.
pub const ACTIVE_WINDOW_SECS: u64 = 300;

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub nodes: u64,
    pub active: u64,
    #[serde(default)]
    pub latency_ms_median: Option<u64>,
    pub uptime_seconds: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Relay NodeId (first 16 hex chars of the ML-KEM-1024 public key).
    pub id: String,
    pub services: Vec<String>,
    /// Human-readable label supplied by the device during a local pairing flow.
    #[serde(default)]
    pub device_name: Option<String>,
    /// Public signing key used to authenticate messages from this node.
    #[serde(default)]
    pub signing_key_hex: Option<String>,
    pub last_seen: u64,
    /// Latence mesurée vers ce nœud, en millisecondes. `None` tant qu'aucune
    /// mesure n'a eu lieu : un nœud fraîchement appairé n'a pas encore de RTT.
    #[serde(default)]
    pub latency_ms: Option<u64>,
}

/// Un nœud est actif s'il a été vu il y a au plus [`ACTIVE_WINDOW_SECS`].
/// `last_seen == 0` (jamais vu) n'est jamais actif.
pub fn is_active(last_seen: u64, now: u64) -> bool {
    last_seen != 0 && now.saturating_sub(last_seen) <= ACTIVE_WINDOW_SECS
}

impl Gateway {
    /// Photographie de l'état réel du réseau à l'instant de l'appel.
    pub async fn snapshot(&self) -> NetworkStatus {
        let nodes = self.nodes.read().await;
        let now = now_secs();
        let active = nodes
            .values()
            .filter(|node| is_active(node.last_seen, now))
            .count() as u64;
        NetworkStatus {
            nodes: nodes.len() as u64,
            active,
            latency_ms_median: self.calculate_latency_median().await,
            uptime_seconds: now.saturating_sub(self.started_at),
        }
    }

    /// Médiane des latences **mesurées** uniquement.
    pub async fn calculate_latency_median(&self) -> Option<u64> {
        let nodes = self.nodes.read().await;
        let mut latencies: Vec<u64> = nodes.values().filter_map(|node| node.latency_ms).collect();

        if latencies.is_empty() {
            return None;
        }

        latencies.sort();
        let len = latencies.len();
        let median = if len.is_multiple_of(2) {
            (latencies[len / 2 - 1] + latencies[len / 2]) / 2
        } else {
            latencies[len / 2]
        };

        Some(median)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Construit un gateway dont le site n'est jamais lu : ces tests ne
    /// touchent que `nodes`, donc `site_dir` peut rester un chemin inerte.
    fn gateway() -> Gateway {
        Gateway::new(PathBuf::from("."))
    }

    async fn add(gateway: &Gateway, id: &str, latency: Option<u64>, last_seen: u64) {
        let mut nodes = gateway.nodes.write().await;
        nodes.insert(
            id.to_string(),
            NodeInfo {
                id: id.to_string(),
                services: vec![],
                device_name: None,
                signing_key_hex: None,
                last_seen,
                latency_ms: latency,
            },
        );
    }

    /// Aucun nœud mesuré → `None`, pas `0` : une absence de mesure n'est pas
    /// une latence nulle.
    #[tokio::test]
    async fn median_is_none_without_measurements() {
        let gateway = gateway();
        assert_eq!(gateway.calculate_latency_median().await, None);

        add(&gateway, "n1", None, now_secs()).await;
        assert_eq!(gateway.calculate_latency_median().await, None);
    }

    /// Nombre impair de mesures → la valeur du milieu.
    #[tokio::test]
    async fn median_odd_count_takes_middle() {
        let gateway = gateway();
        add(&gateway, "n1", Some(30), now_secs()).await;
        add(&gateway, "n2", Some(10), now_secs()).await;
        add(&gateway, "n3", Some(20), now_secs()).await;
        assert_eq!(gateway.calculate_latency_median().await, Some(20));
    }

    /// Nombre pair → moyenne des deux du milieu.
    #[tokio::test]
    async fn median_even_count_averages() {
        let gateway = gateway();
        add(&gateway, "n1", Some(10), now_secs()).await;
        add(&gateway, "n2", Some(20), now_secs()).await;
        add(&gateway, "n3", Some(30), now_secs()).await;
        add(&gateway, "n4", Some(40), now_secs()).await;
        assert_eq!(gateway.calculate_latency_median().await, Some(25));
    }

    /// Les nœuds sans mesure sont ignorés, pas comptés comme zéro.
    #[tokio::test]
    async fn median_ignores_unmeasured_nodes() {
        let gateway = gateway();
        add(&gateway, "measured-a", Some(10), now_secs()).await;
        add(&gateway, "measured-b", Some(30), now_secs()).await;
        add(&gateway, "unmeasured", None, now_secs()).await;
        assert_eq!(gateway.calculate_latency_median().await, Some(20));
    }

    /// Un nœud connu mais silencieux depuis longtemps n'est pas actif.
    #[tokio::test]
    async fn snapshot_counts_only_fresh_nodes_as_active() {
        let gateway = gateway();
        let now = now_secs();
        add(&gateway, "frais", None, now).await;
        add(
            &gateway,
            "vieux",
            None,
            now.saturating_sub(ACTIVE_WINDOW_SECS + 1),
        )
        .await;
        add(&gateway, "jamais-vu", None, 0).await;

        let status = gateway.snapshot().await;
        assert_eq!(status.nodes, 3);
        assert_eq!(status.active, 1);
    }

    #[test]
    fn active_window_boundary_is_inclusive() {
        let now = 10_000;
        assert!(is_active(now - ACTIVE_WINDOW_SECS, now));
        assert!(!is_active(now - ACTIVE_WINDOW_SECS - 1, now));
        assert!(!is_active(0, now));
    }
}
