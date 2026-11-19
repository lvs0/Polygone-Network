//! Rapport de diagnostic agrégé — mesuré, ou explicitement absent.
//!
//! Trois règles, héritées de `status.rs` :
//!
//! - une valeur non mesurée est `null`, jamais `0` ;
//! - ce que la gateway n'interroge pas est déclaré `available: false` avec son
//!   motif, au lieu d'être rempli de zéros crédibles ;
//! - aucune clé publique ou privée n'est recopiée : le rapport agrège des
//!   compteurs et des latences, pas des identités.
//!
//! La gateway ne parle pas au socket du daemon. Les refus d'allocation, les
//! ressources réservables et l'isolation OS restent donc l'affaire de
//! `polygoned status` / `polygone doctor` : ce rapport le dit au lieu de
//! dupliquer un chiffre qu'il ne peut pas vérifier.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::status::{is_active, median_of};
use crate::{Gateway, now_secs};

/// Rapport complet, versionné par sa forme (`v`).
#[derive(Clone, Debug, Serialize)]
pub struct DiagnosticsReport {
    /// Version du schéma de ce rapport : un client peut la vérifier.
    pub v: &'static str,
    pub generated_at: u64,
    pub gateway: GatewayDiagnostics,
    pub network: NetworkDiagnostics,
    pub tasks: TaskDiagnostics,
    /// Ce que la gateway ne mesure pas elle-même, nommé sans détour.
    pub daemon: Unavailable,
    pub refusals: Unavailable,
    /// Limites de portée, pour qu'aucun lecteur ne prenne ce rapport pour une
    /// validation multi-machine.
    pub limitations: Vec<&'static str>,
}

/// Version du rapport. Toute forme incompatible doit changer cette valeur.
pub const DIAGNOSTICS_VERSION: &str = "polygone-diagnostics-v1";

#[derive(Clone, Debug, Serialize)]
pub struct GatewayDiagnostics {
    pub uptime_seconds: u64,
    /// Vrai si les nœuds survivent au redémarrage (fichier configuré).
    pub nodes_persisted: bool,
    /// Vrai si les tâches survivent au redémarrage (fichier configuré).
    pub tasks_persisted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkDiagnostics {
    pub nodes: u64,
    /// Nœuds vus dans les cinq dernières minutes, pas nœuds connus.
    pub active: u64,
    /// Nœuds dont une latence a réellement été rapportée.
    pub latency_measured: u64,
    /// Nœuds connus sans aucune mesure de latence.
    pub latency_unmeasured: u64,
    /// `null` tant qu'aucune mesure n'existe.
    pub latency_ms_median: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskDiagnostics {
    pub total: usize,
    /// Tous les états, y compris à zéro.
    pub by_state: BTreeMap<String, usize>,
    /// Vrai si ce magasin exécute réellement les tâches publiées.
    pub executor_attached: bool,
}

/// Une donnée que ce composant ne peut pas mesurer.
#[derive(Clone, Debug, Serialize)]
pub struct Unavailable {
    pub available: bool,
    pub reason: &'static str,
}

impl Gateway {
    /// Photographie agrégée, prise sous les verrous existants.
    pub async fn diagnostics(&self) -> DiagnosticsReport {
        let now = now_secs();

        let (nodes, active, latency_measured, latency_ms_median) = {
            let nodes = self.nodes.read().await;
            let mut latencies: Vec<u64> =
                nodes.values().filter_map(|node| node.latency_ms).collect();
            let measured = latencies.len() as u64;
            let active = nodes
                .values()
                .filter(|node| is_active(node.last_seen, now))
                .count() as u64;
            (
                nodes.len() as u64,
                active,
                measured,
                median_of(&mut latencies),
            )
        };

        let by_state = self.tasks.counts_by_state(now).await;
        let total = by_state.values().sum();
        let executor_attached = self.tasks.executor_attached().await;

        DiagnosticsReport {
            v: DIAGNOSTICS_VERSION,
            generated_at: now,
            gateway: GatewayDiagnostics {
                uptime_seconds: now.saturating_sub(self.started_at),
                nodes_persisted: self.nodes_file.is_some(),
                tasks_persisted: self.tasks.persisted().await,
            },
            network: NetworkDiagnostics {
                nodes,
                active,
                latency_measured,
                latency_unmeasured: nodes.saturating_sub(latency_measured),
                latency_ms_median,
            },
            tasks: TaskDiagnostics {
                total,
                by_state,
                executor_attached,
            },
            daemon: Unavailable {
                available: false,
                reason: "la gateway n'interroge pas le socket du daemon ; utilisez `polygoned status` pour les réserves et `polygone doctor` pour l'isolation",
            },
            refusals: Unavailable {
                available: false,
                reason: "les compteurs de refus vivent dans le daemon ; `polygoned status` les publie, aucun zéro n'est inventé ici",
            },
            limitations: vec![
                "aucun test entre deux machines physiques : ce rapport décrit un processus local",
                "une latence est rapportée par le pair et signée ; la gateway garantit sa fraîcheur, pas son honnêteté",
                "l'exécution des tâches est locale et mono-nœud : aucun placement distribué",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeInfo;
    use crate::http::HttpRequest;
    use std::path::PathBuf;

    fn temp_tasks_path(label: &str) -> (PathBuf, PathBuf) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "polygone-gateway-diagnostics-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tasks.json");
        (dir, path)
    }

    fn task_body() -> String {
        r#"{"owner":"ui-user","kind":"echo","input_hex":"68656c6c6f","deadline_in_seconds":300}"#
            .to_string()
    }

    fn get(path: &str) -> HttpRequest {
        HttpRequest::new("GET", path, Vec::new())
    }

    #[tokio::test]
    async fn diagnostics_reports_measured_network_and_tasks_without_keys() {
        let (dir, tasks_file) = temp_tasks_path("measured");
        let gateway = Gateway::with_state_files(PathBuf::from("."), None, Some(tasks_file));
        let now = now_secs();
        {
            let mut nodes = gateway.nodes.write().await;
            nodes.insert(
                "node-a".into(),
                NodeInfo {
                    id: "node-a".into(),
                    services: vec!["msg".into()],
                    device_name: Some("ancien-portable".into()),
                    signing_key_hex: Some("deadbeefdeadbeef".into()),
                    last_seen: now,
                    latency_ms: Some(42),
                },
            );
            nodes.insert(
                "node-b".into(),
                NodeInfo {
                    id: "node-b".into(),
                    services: vec![],
                    device_name: None,
                    signing_key_hex: None,
                    last_seen: now,
                    latency_ms: None,
                },
            );
        }

        let created = gateway
            .route(HttpRequest::new(
                "POST",
                "/api/v1/tasks",
                task_body().into_bytes(),
            ))
            .await;
        assert_eq!(created.status, 201);

        let response = gateway.route(get("/api/v1/diagnostics")).await;
        assert_eq!(response.status, 200);
        let report: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(report["v"], DIAGNOSTICS_VERSION);
        assert_eq!(report["network"]["nodes"], 2);
        assert_eq!(report["network"]["active"], 2);
        assert_eq!(report["network"]["latency_measured"], 1);
        assert_eq!(report["network"]["latency_unmeasured"], 1);
        assert_eq!(report["network"]["latency_ms_median"], 42);
        // L'exécuteur attaché exécute réellement la tâche echo.
        assert_eq!(report["tasks"]["by_state"]["succeeded"], 1);
        assert_eq!(report["tasks"]["by_state"]["published"], 0);
        assert_eq!(report["tasks"]["executor_attached"], true);
        assert_eq!(report["daemon"]["available"], false);
        assert_eq!(report["refusals"]["available"], false);

        let raw = String::from_utf8(response.body).unwrap();
        assert!(
            !raw.contains("deadbeef"),
            "aucune clé de signature ne doit apparaître dans le rapport"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn diagnostics_never_invents_a_latency_or_a_refusal_count() {
        let gateway = Gateway::new(PathBuf::from("."));
        let report = gateway.diagnostics().await;

        assert_eq!(report.network.nodes, 0);
        assert_eq!(report.network.latency_ms_median, None);
        assert_eq!(report.network.latency_unmeasured, 0);
        assert!(!report.daemon.available);
        assert!(!report.refusals.available);
        assert!(!report.tasks.executor_attached);
        // Tous les états existent, même sans aucune tâche.
        assert_eq!(report.tasks.by_state.len(), 10);
        assert!(report.tasks.by_state.values().all(|count| *count == 0));
    }
}
