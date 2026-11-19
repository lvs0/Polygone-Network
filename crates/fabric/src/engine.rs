//! Moteur d'exécution bidirectionnel.
//!
//! Le moteur ne fait pas confiance à un résultat parce qu'il arrive : il le
//! vérifie (hash BLAKE3, signature ML-DSA-65, `NodeId` dérivé, identifiant de
//! tâche). Un échec de vérification n'est **jamais** converti en succès ; le
//! nœud fautif est exclu et le placement est recalculé pour une autre tentative.

use std::time::Instant;

use polygone_core::sign::{node_id_from_sign_pk_hex, KeyPair};
use polygone_core::task::{TaskResult, TaskSpec};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::learning::LearningStore;
use crate::model::{NodeCapabilities, TaskEnvelope};
use crate::planner::{self, Placement, PlacementPlan, PlacementPolicy};

/// Erreur d'un transport, distinguée pour que l'appelant sache si réessayer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    #[error("nœud indisponible : {0}")]
    Unavailable(String),
    #[error("refusé ({code}) : {message}")]
    Refused { code: String, message: String },
    #[error("message malformé : {0}")]
    Malformed(String),
    #[error("exécution impossible : {0}")]
    Execution(String),
    #[error("E/S : {0}")]
    Io(String),
}

/// Contrat minimal d'un réseau de nœuds.
///
/// `dispatch` rend le résultat **brut** : c'est le moteur qui le vérifie. Un
/// transport ne peut donc pas promouvoir un résultat non signé en succès.
pub trait Transport {
    fn list_nodes(&self) -> Vec<NodeCapabilities>;
    fn dispatch(&self, node: &str, envelope: &TaskEnvelope) -> Result<TaskResult, TransportError>;
}

/// Trace d'une tentative, lisible par une interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub node: String,
    pub success: bool,
    pub latency_ms: u64,
}

/// Résultat complet d'une exécution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub task_id: String,
    /// Vrai **seulement** si un [`TaskResult`] a été vérifié.
    pub verified: bool,
    pub attempts: u32,
    pub placement: Placement,
    pub result: Option<TaskResult>,
    pub observations: Vec<ObservationRecord>,
    pub error: Option<String>,
}

/// Moteur : un nœud local, ses clés, sa politique et sa connaissance locale.
pub struct FabricEngine {
    pub local: NodeCapabilities,
    keypair: KeyPair,
    pub policy: PlacementPolicy,
    pub learning: LearningStore,
}

impl FabricEngine {
    pub fn new(
        local: NodeCapabilities,
        keypair: KeyPair,
        policy: PlacementPolicy,
    ) -> Result<Self, String> {
        local.validate()?;
        let derived = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
            .ok_or_else(|| "clé ML-DSA-65 invalide".to_string())?;
        if derived != local.node_id {
            return Err(format!(
                "node_id local {} ne dérive pas de la clé {}",
                local.node_id, derived
            ));
        }
        Ok(Self {
            local,
            keypair,
            policy,
            learning: LearningStore::new(),
        })
    }

    /// Exécute une tâche, localement ou à distance, avec reprise sur un autre
    /// nœud après échec vérifié.
    pub fn execute(
        &mut self,
        envelope: &TaskEnvelope,
        now: u64,
        transport: Option<&dyn Transport>,
    ) -> ExecutionOutcome {
        let task = &envelope.task;
        if let Err(error) = task.validate(now) {
            return ExecutionOutcome {
                task_id: task.id.as_str().to_string(),
                verified: false,
                attempts: 0,
                placement: Placement::Refused {
                    code: "invalid-task".to_string(),
                    message: error.to_string(),
                },
                result: None,
                observations: Vec::new(),
                error: Some(error.to_string()),
            };
        }

        let max_attempts = task
            .constraints
            .max_attempts
            .min(self.policy.max_attempts)
            .max(1);
        let mut excluded: Vec<String> = Vec::new();
        let mut observations: Vec<ObservationRecord> = Vec::new();
        let mut last_plan: Option<PlacementPlan> = None;

        for attempt in 1..=max_attempts {
            let candidates = transport
                .map(|transport| transport.list_nodes())
                .unwrap_or_default();
            let candidates: Vec<NodeCapabilities> = candidates
                .into_iter()
                .map(|mut node| {
                    node.failure_rate_permille = self
                        .learning
                        .failure_permille(&node.node_id, node.failure_rate_permille);
                    node
                })
                .collect();

            let plan = planner::plan(
                task,
                envelope.scope,
                envelope.global_consent,
                &envelope.fabric,
                &self.local,
                &candidates,
                &self.policy,
                &excluded,
                now,
            );
            let placement = plan.placement.clone();
            last_plan = Some(plan);

            match placement {
                Placement::Refused { code, message } => {
                    return ExecutionOutcome {
                        task_id: task.id.as_str().to_string(),
                        verified: false,
                        attempts: attempt,
                        placement: Placement::Refused {
                            code,
                            message: message.clone(),
                        },
                        result: None,
                        observations,
                        error: Some(message),
                    };
                }
                Placement::Local => {
                    let started = Instant::now();
                    match self.run_local(task, now) {
                        Ok(result) => {
                            let latency_ms = started.elapsed().as_millis() as u64;
                            self.learning.observe(&self.local.node_id, true, latency_ms);
                            observations.push(ObservationRecord {
                                node: self.local.node_id.clone(),
                                success: true,
                                latency_ms,
                            });
                            return ExecutionOutcome {
                                task_id: task.id.as_str().to_string(),
                                verified: true,
                                attempts: attempt,
                                placement: Placement::Local,
                                result: Some(result),
                                observations,
                                error: None,
                            };
                        }
                        Err(error) => {
                            let latency_ms = started.elapsed().as_millis() as u64;
                            self.learning
                                .observe(&self.local.node_id, false, latency_ms);
                            observations.push(ObservationRecord {
                                node: self.local.node_id.clone(),
                                success: false,
                                latency_ms,
                            });
                            // Une erreur locale est déterministe : réessayer
                            // sur la même machine ne changerait rien.
                            return ExecutionOutcome {
                                task_id: task.id.as_str().to_string(),
                                verified: false,
                                attempts: attempt,
                                placement: Placement::Local,
                                result: None,
                                observations,
                                error: Some(error),
                            };
                        }
                    }
                }
                Placement::Remote { node } => {
                    let Some(transport) = transport else {
                        return ExecutionOutcome {
                            task_id: task.id.as_str().to_string(),
                            verified: false,
                            attempts: attempt,
                            placement: Placement::Remote { node: node.clone() },
                            result: None,
                            observations,
                            error: Some("aucun transport pour joindre le nœud distant".to_string()),
                        };
                    };
                    let started = Instant::now();
                    let dispatched = transport.dispatch(&node, envelope);
                    let latency_ms = started.elapsed().as_millis() as u64;
                    match dispatched {
                        Ok(result) => match self.verify_remote(task, &node, &result) {
                            Ok(()) => {
                                self.learning.observe(&node, true, latency_ms);
                                observations.push(ObservationRecord {
                                    node: node.clone(),
                                    success: true,
                                    latency_ms,
                                });
                                return ExecutionOutcome {
                                    task_id: task.id.as_str().to_string(),
                                    verified: true,
                                    attempts: attempt,
                                    placement: Placement::Remote { node: node.clone() },
                                    result: Some(result),
                                    observations,
                                    error: None,
                                };
                            }
                            Err(error) => {
                                self.learning.observe(&node, false, latency_ms);
                                observations.push(ObservationRecord {
                                    node: node.clone(),
                                    success: false,
                                    latency_ms,
                                });
                                excluded.push(node.clone());
                                if attempt == max_attempts {
                                    return ExecutionOutcome {
                                        task_id: task.id.as_str().to_string(),
                                        verified: false,
                                        attempts: attempt,
                                        placement: Placement::Remote { node },
                                        result: None,
                                        observations,
                                        error: Some(error),
                                    };
                                }
                            }
                        },
                        Err(error) => {
                            self.learning.observe(&node, false, latency_ms);
                            observations.push(ObservationRecord {
                                node: node.clone(),
                                success: false,
                                latency_ms,
                            });
                            excluded.push(node.clone());
                            if attempt == max_attempts {
                                return ExecutionOutcome {
                                    task_id: task.id.as_str().to_string(),
                                    verified: false,
                                    attempts: attempt,
                                    placement: Placement::Remote { node },
                                    result: None,
                                    observations,
                                    error: Some(error.to_string()),
                                };
                            }
                        }
                    }
                }
            }
        }

        let placement = last_plan
            .map(|plan| plan.placement)
            .unwrap_or(Placement::Refused {
                code: "exhausted".to_string(),
                message: "tentatives épuisées".to_string(),
            });
        ExecutionOutcome {
            task_id: task.id.as_str().to_string(),
            verified: false,
            attempts: max_attempts,
            placement,
            result: None,
            observations,
            error: Some("tentatives épuisées".to_string()),
        }
    }

    fn run_local(&self, task: &TaskSpec, now: u64) -> Result<TaskResult, String> {
        let output = task
            .kind
            .execute(&task.input, task.constraints.permissions.max_output_bytes)
            .map_err(|error| error.to_string())?;
        let result = TaskResult::signed(
            task.id.clone(),
            output,
            self.local.node_id.clone(),
            now,
            &self.keypair,
        )
        .map_err(|error| error.to_string())?;
        result.verify().map_err(|error| error.to_string())?;
        Ok(result)
    }

    fn verify_remote(
        &self,
        task: &TaskSpec,
        node: &str,
        result: &TaskResult,
    ) -> Result<(), String> {
        if result.task_id != task.id {
            return Err(format!(
                "résultat de la tâche {} présenté pour {}",
                result.task_id, task.id
            ));
        }
        if result.producer != node {
            return Err(format!(
                "producteur {} différent du nœud assigné {}",
                result.producer, node
            ));
        }
        result.verify().map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Scope, WIRE_VERSION};
    use crate::transport::{MemoryNode, MemoryTransport};
    use polygone_core::sign::node_id_from_sign_pk_hex;
    use polygone_core::task::{
        TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRequirements, TaskSpec,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn capabilities(node_id: &str, cores: u32, load: u16) -> NodeCapabilities {
        NodeCapabilities {
            node_id: node_id.to_string(),
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
            rtt_ms: 1,
            bandwidth_kbps: 1_000_000,
            failure_rate_permille: 0,
            available: true,
        }
    }

    fn task(kind: TaskKind, input: &[u8], cores: u32, attempts: u32) -> TaskSpec {
        let id = format!(
            "task_engine_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        TaskSpec {
            id: TaskId::parse(id).unwrap(),
            owner: "owner".to_string(),
            kind,
            input: input.to_vec(),
            requirements: TaskRequirements {
                cpu_cores: cores,
                ram_bytes: 1024,
                storage_bytes: 0,
                gpu_vram_bytes: 0,
            },
            constraints: TaskConstraints {
                deadline_unix: crate::now_unix() + 300,
                max_attempts: attempts,
                permissions: TaskPermissions::default(),
            },
        }
    }

    fn envelope(task: TaskSpec) -> TaskEnvelope {
        TaskEnvelope {
            v: WIRE_VERSION,
            scope: Scope::Local,
            global_consent: false,
            fabric: "maison".to_string(),
            requester: "requester".to_string(),
            task,
        }
    }

    fn engine(cores: u32, load: u16) -> FabricEngine {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        FabricEngine::new(
            capabilities(&id, cores, load),
            keypair,
            PlacementPolicy::default(),
        )
        .unwrap()
    }

    fn remote_node(cores: u32, load: u16) -> MemoryNode {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        MemoryNode::new(capabilities(&id, cores, load), keypair).unwrap()
    }

    #[test]
    fn local_execution_is_verified_and_signed() {
        let mut engine = engine(8, 0);
        let outcome = engine.execute(
            &envelope(task(TaskKind::Blake3, b"local", 1, 1)),
            crate::now_unix(),
            None,
        );
        assert!(outcome.verified, "{outcome:?}");
        assert_eq!(outcome.placement, Placement::Local);
        let result = outcome.result.unwrap();
        result.verify().expect("signature");
        assert_eq!(result.output, blake3::hash(b"local").to_hex().as_bytes());
    }

    #[test]
    fn a_tampered_remote_result_is_never_a_success() {
        let mut engine = engine(1, 980);
        let node = remote_node(8, 0).tampering();
        let node_id = node.capabilities.node_id.clone();
        let transport = MemoryTransport::new(vec![node]);
        let outcome = engine.execute(
            &envelope(task(TaskKind::Echo, b"secret", 2, 1)),
            crate::now_unix(),
            Some(&transport),
        );
        assert!(!outcome.verified, "{outcome:?}");
        assert!(outcome.result.is_none(), "aucun résultat accepté");
        assert!(
            outcome.error.unwrap().contains("hash"),
            "le motif doit nommer l'intégrité"
        );
        assert!(engine.learning.failure_permille(&node_id, 0) > 0);
    }

    #[test]
    fn a_failed_node_is_replaced_by_another() {
        let mut engine = engine(1, 980);
        let first = remote_node(64, 0).failing();
        let first_id = first.capabilities.node_id.clone();
        let mut second = remote_node(8, 10);
        second.capabilities.rtt_ms = 100;
        second.capabilities.bandwidth_kbps = 10_000;
        let second_id = second.capabilities.node_id.clone();
        let transport = MemoryTransport::new(vec![first, second]);
        let outcome = engine.execute(
            &envelope(task(TaskKind::Echo, b"failover", 2, 2)),
            crate::now_unix(),
            Some(&transport),
        );
        assert!(outcome.verified, "{outcome:?}");
        assert_eq!(outcome.attempts, 2);
        assert!(outcome
            .observations
            .iter()
            .any(|o| o.node == first_id && !o.success));
        assert!(outcome
            .observations
            .iter()
            .any(|o| o.node == second_id && o.success));
        assert_eq!(outcome.result.unwrap().producer, second_id);
    }

    #[test]
    fn exhaustion_returns_no_result() {
        let mut engine = engine(1, 980);
        let transport = MemoryTransport::new(vec![remote_node(8, 0).failing()]);
        let outcome = engine.execute(
            &envelope(task(TaskKind::Echo, b"x", 2, 1)),
            crate::now_unix(),
            Some(&transport),
        );
        assert!(!outcome.verified);
        assert!(outcome.result.is_none());
        assert!(outcome.error.is_some());
    }

    #[test]
    fn an_invalid_task_is_refused_before_any_execution() {
        let mut engine = engine(8, 0);
        let mut task = task(TaskKind::Echo, b"x", 1, 1);
        task.constraints.deadline_unix = crate::now_unix();
        let outcome = engine.execute(&envelope(task), crate::now_unix(), None);
        assert!(!outcome.verified);
        assert_eq!(outcome.attempts, 0);
    }
}
