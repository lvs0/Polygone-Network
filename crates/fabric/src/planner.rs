//! Planificateur de placement — local d'abord, distant seulement si c'est
//! réellement moins cher.
//!
//! Le modèle de coût est **volontairement simple et déterministe** : il ne
//! prétend pas prédire la performance réelle. Il applique trois règles
//! vérifiables :
//!
//! 1. une tâche ne quitte jamais son périmètre autorisé ;
//! 2. un nœud distant n'est choisi que si son coût total estimé (transfert
//!    aller-retour + exécution + latence) est inférieur à la marge près au
//!    coût local ;
//! 3. l'échéance doit être atteignable, sinon la tâche est refusée.
//!
//! Les chiffres du modèle ne sont pas des benchmarks. Le seul chiffre mesuré
//! est la latence observée par l'exécuteur, enregistrée dans
//! [`crate::learning`].

use crate::model::{NodeCapabilities, Scope};
use polygone_core::task::{TaskKind, TaskSpec};
use serde::{Deserialize, Serialize};

/// Politique de placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementPolicy {
    /// Le distant doit coûter au plus cette fraction (en millièmes) du local.
    /// 900 = il doit être au moins 10 % moins cher.
    pub margin_permille: u16,
    /// Nombre maximal de tentatives, plafonné par la tâche elle-même.
    pub max_attempts: u32,
    /// Attente locale déjà connue, en millisecondes (file de la machine).
    pub local_queue_ms: u64,
}

impl Default for PlacementPolicy {
    fn default() -> Self {
        Self {
            margin_permille: 900,
            max_attempts: 3,
            local_queue_ms: 0,
        }
    }
}

/// Décision de placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "where", rename_all = "snake_case")]
pub enum Placement {
    Local,
    Remote { node: String },
    Refused { code: String, message: String },
}

/// Estimation d'un candidat distant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEstimate {
    pub node: String,
    pub total_ms: u64,
    pub transfer_ms: u64,
    pub exec_ms: u64,
    pub rtt_ms: u32,
}

/// Plan complet, lisible par une interface sans reconstruire le calcul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementPlan {
    pub placement: Placement,
    pub local_estimate_ms: Option<u64>,
    pub remote_estimates: Vec<RemoteEstimate>,
    pub reason: String,
}

/// Estimation d'exécution, en millisecondes.
///
/// `Echo` et `Blake3` sont les deux familles déterministes du modèle de tâche.
/// Le coût par Mio est un ordre de grandeur de modèle, pas une mesure. La
/// charge divise le débit utile ; une charge > 900 ‰ est traitée comme 900 ‰
/// pour ne jamais produire une division par zéro.
pub fn estimate_execution_ms(
    kind: TaskKind,
    input_len: usize,
    cores: u32,
    load_permille: u16,
) -> u64 {
    let cores = u64::from(cores.max(1));
    let mib = (input_len as u64).div_ceil(1_048_576);
    let (base, per_mib) = match kind {
        TaskKind::Echo => (2u64, 8u64),
        TaskKind::Blake3 => (1u64, 2u64),
    };
    let idle = base.saturating_add(per_mib.saturating_mul(mib.max(1)) / cores);
    let free = 1000u64
        .saturating_sub(u64::from(load_permille.min(900)))
        .max(100);
    idle.saturating_mul(1000) / free
}

fn output_len(kind: TaskKind, input_len: usize) -> u64 {
    match kind {
        TaskKind::Echo => input_len as u64,
        TaskKind::Blake3 => 64,
    }
}

fn estimate_remote(task: &TaskSpec, node: &NodeCapabilities, now: u64) -> RemoteEstimate {
    let exec_ms = estimate_execution_ms(
        task.kind,
        task.input.len(),
        task.requirements.cpu_cores,
        node.load_permille,
    );
    let bits = (task.input.len() as u64)
        .saturating_add(output_len(task.kind, task.input.len()))
        .saturating_mul(8);
    let bandwidth_kbps = u64::from(node.bandwidth_kbps.max(1));
    let transfer_ms = bits / bandwidth_kbps; // kbit/s = 1 bit/ms
    let total_ms = transfer_ms
        .saturating_add(exec_ms)
        .saturating_add(u64::from(node.rtt_ms).saturating_mul(2));
    let _ = now;
    RemoteEstimate {
        node: node.node_id.clone(),
        total_ms,
        transfer_ms,
        exec_ms,
        rtt_ms: node.rtt_ms,
    }
}

fn deadline_reached(now: u64, estimate_ms: u64, deadline_unix: u64) -> bool {
    let seconds = estimate_ms.saturating_add(999) / 1000;
    now.saturating_add(seconds) <= deadline_unix
}

fn scope_allows(
    scope: Scope,
    global_consent: bool,
    fabric: &str,
    node: &NodeCapabilities,
) -> Result<(), String> {
    match scope {
        Scope::Local => {
            if node.scope != Scope::Local {
                return Err("nœud hors du périmètre local".to_string());
            }
            if node.local_fabric != fabric {
                return Err("réseau local différent".to_string());
            }
            Ok(())
        }
        Scope::Global => {
            if !global_consent {
                return Err("consentement global absent côté demandeur".to_string());
            }
            if node.scope != Scope::Global || !node.global_consent {
                return Err("le nœud n'a pas consenti au périmètre global".to_string());
            }
            Ok(())
        }
    }
}

fn refused(code: &str, message: impl Into<String>) -> PlacementPlan {
    let message = message.into();
    PlacementPlan {
        placement: Placement::Refused {
            code: code.to_string(),
            message: message.clone(),
        },
        local_estimate_ms: None,
        remote_estimates: Vec::new(),
        reason: message,
    }
}

/// Choisit où exécuter `task`.
///
/// `excluded` contient les nœuds déjà essayés et échoués pendant cette
/// exécution ; ils ne sont jamais re-proposés dans la même tentative.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    task: &TaskSpec,
    scope: Scope,
    global_consent: bool,
    fabric: &str,
    local: &NodeCapabilities,
    candidates: &[NodeCapabilities],
    policy: &PlacementPolicy,
    excluded: &[String],
    now: u64,
) -> PlacementPlan {
    let deadline = task.constraints.deadline_unix;
    if deadline <= now {
        return refused("deadline", "échéance déjà passée");
    }

    let local_feasible = local.can_run_locally(task).is_ok();
    let local_total = local_feasible.then(|| {
        policy.local_queue_ms.saturating_add(estimate_execution_ms(
            task.kind,
            task.input.len(),
            task.requirements.cpu_cores,
            local.load_permille,
        ))
    });
    let local_in_time = local_total.filter(|total| deadline_reached(now, *total, deadline));

    let mut estimates: Vec<RemoteEstimate> = candidates
        .iter()
        .filter(|node| node.node_id != local.node_id)
        .filter(|node| !excluded.iter().any(|excluded| excluded == &node.node_id))
        .filter(|node| scope_allows(scope, global_consent, fabric, node).is_ok())
        .filter(|node| node.can_accept_remote(task).is_ok())
        .map(|node| estimate_remote(task, node, now))
        .filter(|estimate| deadline_reached(now, estimate.total_ms, deadline))
        .collect();
    estimates.sort_by_key(|estimate| (estimate.total_ms, estimate.node.clone()));

    match (local_in_time, estimates.first()) {
        (Some(local_cost), Some(remote)) => {
            let remote_total = remote.total_ms;
            let remote_node = remote.node.clone();
            let remote_is_cheaper = remote_total.saturating_mul(1000)
                < local_cost.saturating_mul(u64::from(policy.margin_permille));
            let (placement, reason) = if remote_is_cheaper {
                (
                    Placement::Remote { node: remote_node },
                    format!(
                        "distant retenu : {} ms estimés contre {} ms en local (marge {} ‰)",
                        remote_total, local_cost, policy.margin_permille
                    ),
                )
            } else {
                (
                    Placement::Local,
                    format!(
                        "local retenu : {} ms estimés, le transfert distant ne bat pas la marge",
                        local_cost
                    ),
                )
            };
            PlacementPlan {
                placement,
                local_estimate_ms: Some(local_cost),
                remote_estimates: estimates,
                reason,
            }
        }
        (Some(local_cost), None) => PlacementPlan {
            placement: Placement::Local,
            local_estimate_ms: Some(local_cost),
            remote_estimates: estimates,
            reason: "aucun candidat distant admissible : exécution locale".to_string(),
        },
        (None, Some(remote)) => PlacementPlan {
            placement: Placement::Remote {
                node: remote.node.clone(),
            },
            local_estimate_ms: None,
            remote_estimates: estimates,
            reason: "local indisponible ou hors délai : seul un candidat distant reste".to_string(),
        },
        (None, None) => refused(
            "no-candidate",
            "aucun nœud local ou distant ne peut exécuter la tâche dans le périmètre et le délai",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use polygone_core::task::{
        TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRequirements,
    };

    const NOW: u64 = 1_800_000_000;

    fn task(kind: TaskKind, input_len: usize, cores: u32) -> TaskSpec {
        TaskSpec {
            id: TaskId::parse("task_planner_1").unwrap(),
            owner: "owner".to_string(),
            kind,
            input: vec![0u8; input_len],
            requirements: TaskRequirements {
                cpu_cores: cores,
                ram_bytes: 1024,
                storage_bytes: 0,
                gpu_vram_bytes: 0,
            },
            constraints: TaskConstraints {
                deadline_unix: NOW + 300,
                max_attempts: 3,
                permissions: TaskPermissions::default(),
            },
        }
    }

    fn node(id: &str, cores: u32, load: u16, rtt: u32, bandwidth: u32) -> NodeCapabilities {
        NodeCapabilities {
            node_id: id.to_string(),
            scope: Scope::Local,
            global_consent: false,
            local_fabric: "maison".to_string(),
            cpu_cores: cores,
            ram_bytes: 1024 * 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
            load_permille: load,
            battery_percent: None,
            thermal_celsius: None,
            user_active: false,
            rtt_ms: rtt,
            bandwidth_kbps: bandwidth,
            failure_rate_permille: 0,
            available: true,
        }
    }

    #[test]
    fn a_small_task_stays_local_when_transfer_costs_more() {
        let local = node("local", 8, 50, 0, 1_000_000);
        let remote = node("remote", 32, 10, 100, 1_000);
        let plan = plan(
            &task(TaskKind::Echo, 1024, 1),
            Scope::Local,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert_eq!(plan.placement, Placement::Local, "{plan:?}");
        assert_eq!(plan.remote_estimates.len(), 1);
    }

    #[test]
    fn a_busy_local_machine_sends_work_away() {
        let local = node("local", 1, 980, 0, 1_000_000);
        let remote = node("remote", 8, 10, 1, 1_000_000);
        let plan = plan(
            &task(TaskKind::Blake3, 1024, 2),
            Scope::Local,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert_eq!(
            plan.placement,
            Placement::Remote {
                node: "remote".to_string()
            },
            "{plan:?}"
        );
    }

    #[test]
    fn global_requires_consent_on_both_sides() {
        let local = node("local", 8, 0, 0, 1_000_000);
        let mut remote = node("remote", 64, 0, 1, 1_000_000);
        remote.scope = Scope::Global;
        remote.global_consent = false;
        let first = plan(
            &task(TaskKind::Echo, 1024, 1),
            Scope::Global,
            true,
            "maison",
            &local,
            &[remote.clone()],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert!(first.remote_estimates.is_empty(), "{first:?}");
        assert_eq!(first.placement, Placement::Local);

        remote.global_consent = true;
        let second = plan(
            &task(TaskKind::Echo, 1024, 1),
            Scope::Global,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert!(second.remote_estimates.is_empty(), "{second:?}");
    }

    #[test]
    fn a_local_task_never_crosses_to_another_fabric() {
        let local = node("local", 8, 0, 0, 1_000_000);
        let mut remote = node("remote", 64, 0, 1, 1_000_000);
        remote.local_fabric = "voisin".to_string();
        let plan = plan(
            &task(TaskKind::Echo, 1024, 1),
            Scope::Local,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert!(plan.remote_estimates.is_empty(), "{plan:?}");
    }

    #[test]
    fn user_activity_on_the_remote_node_removes_it() {
        let local = node("local", 1, 980, 0, 1_000_000);
        let mut remote = node("remote", 64, 0, 1, 1_000_000);
        remote.user_active = true;
        let plan = plan(
            &task(TaskKind::Echo, 1024, 2),
            Scope::Local,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert_eq!(
            plan.placement,
            Placement::Refused {
                code: "no-candidate".to_string(),
                message:
                    "aucun nœud local ou distant ne peut exécuter la tâche dans le périmètre et le délai"
                        .to_string()
            }
        );
    }

    #[test]
    fn excluded_nodes_are_not_reproposed() {
        let local = node("local", 1, 980, 0, 1_000_000);
        let remote = node("remote", 8, 0, 1, 1_000_000);
        let plan = plan(
            &task(TaskKind::Echo, 1024, 2),
            Scope::Local,
            false,
            "maison",
            &local,
            &[remote],
            &PlacementPolicy::default(),
            &["remote".to_string()],
            NOW,
        );
        assert!(plan.remote_estimates.is_empty(), "{plan:?}");
    }

    #[test]
    fn a_task_past_its_deadline_is_refused() {
        let local = node("local", 8, 0, 0, 1_000_000);
        let mut t = task(TaskKind::Echo, 10, 1);
        t.constraints.deadline_unix = NOW;
        let plan = plan(
            &t,
            Scope::Local,
            false,
            "maison",
            &local,
            &[],
            &PlacementPolicy::default(),
            &[],
            NOW,
        );
        assert!(matches!(plan.placement, Placement::Refused { .. }));
    }
}
