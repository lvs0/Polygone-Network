//! Modèle de tâche — spec, état, résultat, transitions.
//!
//! Ce module ne fait **aucune E/S**, ne connaît ni socket ni disque, et ne
//! décide pas où une tâche s'exécute. Il fixe ce qui est vrai d'une tâche :
//!
//! - une spécification validée (propriétaire, type, entrée bornée, besoins,
//!   permissions, échéance) ;
//! - une machine à états explicite, où chaque transition est nommée et où un
//!   état terminal ne change plus ;
//! - un résultat dont l'intégrité est vérifiable (`BLAKE3`) et dont l'auteur
//!   doit être le nœud assigné.
//!
//! **Ce module ne promet pas l'exécution.** Publier une tâche ne la fait pas
//! tourner. Le jour où un exécuteur existe, il devra passer par ces transitions
//! et fournir un [`TaskResult`] vérifiable ; aucune autre voie ne doit pouvoir
//! marquer une tâche comme réussie.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Entrée maximale d'une tâche : 64 Kio.
///
/// Une tâche transporte une entrée, pas un fichier. Les charges lourdes
/// passeront plus tard par le magasin de blocs (`blocks.rs`), pas par un
/// message de contrôle.
pub const MAX_TASK_INPUT_BYTES: usize = 64 * 1024;
/// Sortie maximale d'une tâche : 1 Mio.
pub const MAX_TASK_OUTPUT_BYTES: u64 = 1024 * 1024;
/// Nombre de cœurs demandables au maximum.
pub const MAX_TASK_CPU_CORES: u32 = 1024;
/// RAM, stockage et VRAM demandables au maximum (1 Tio chacun).
pub const MAX_TASK_RESOURCE_BYTES: u64 = 1 << 40;
/// Horizon maximal d'une échéance : 7 jours.
pub const MAX_TASK_DEADLINE_HORIZON_SECS: u64 = 7 * 24 * 60 * 60;
/// Nombre de tentatives maximal.
pub const MAX_TASK_ATTEMPTS: u32 = 8;
/// Longueur maximale d'un identifiant de tâche.
pub const MAX_TASK_ID_LEN: usize = 80;
/// Longueur maximale du propriétaire.
pub const MAX_OWNER_LEN: usize = 128;
/// Longueur maximale d'un identifiant de producteur (nœud).
pub const MAX_PRODUCER_LEN: usize = 128;

/// Erreurs du modèle de tâche.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskError {
    #[error("identifiant de tâche invalide : {0}")]
    InvalidTaskId(String),
    #[error("propriétaire vide")]
    EmptyOwner,
    #[error("propriétaire trop long (maximum {MAX_OWNER_LEN} octets)")]
    OwnerTooLong,
    #[error("propriétaire contient un caractère de contrôle")]
    OwnerControlCharacter,
    #[error("entrée de tâche trop grande ({size} octets, maximum {MAX_TASK_INPUT_BYTES})")]
    InputTooLarge { size: usize },
    #[error("exigence invalide : {0}")]
    InvalidRequirement(&'static str),
    #[error("contrainte invalide : {0}")]
    InvalidConstraint(&'static str),
    #[error("échéance déjà passée")]
    DeadlineInPast,
    #[error("échéance au-delà de l'horizon maximal")]
    DeadlineTooFar,
    #[error("permission non supportée par cette famille de tâches : {0}")]
    UnsupportedPermission(&'static str),
    #[error("transition interdite : {from} → {to}")]
    IllegalTransition { from: TaskState, to: TaskState },
    #[error("état terminal : {0}")]
    TerminalState(TaskState),
    #[error("aucun nœud assigné")]
    MissingAssignment,
    #[error("nœud assigné invalide : {0}")]
    InvalidProducer(String),
    #[error("résultat absent")]
    MissingResult,
    #[error("résultat trop grand ({size} octets, maximum {MAX_TASK_OUTPUT_BYTES})")]
    ResultTooLarge { size: usize },
    #[error("résultat produit dans le futur")]
    ResultFromFuture,
    #[error("hash de résultat incohérent : attendu {expected}, calculé {computed}")]
    ResultHashMismatch { expected: String, computed: String },
    #[error("producteur du résultat ({producer}) différent du nœud assigné ({assigned})")]
    ProducerMismatch { producer: String, assigned: String },
    #[error("échéance non atteinte")]
    DeadlineNotReached,
    #[error("motif d'échec vide")]
    EmptyFailureReason,
    #[error("tentatives épuisées ({attempts}/{max})")]
    AttemptsExhausted { attempts: u32, max: u32 },
}

/// Identifiant de tâche, validé à la construction et à la désérialisation.
///
/// Format libre mais borné : caractères ASCII alphanumériques, `-` et `_`.
/// Le générateur vit dans la couche qui crée la tâche (la gateway), pas ici :
/// `core` ne dépend pas d'une source d'aléa pour ce type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TaskId(String);

impl TaskId {
    pub fn parse(value: impl Into<String>) -> Result<Self, TaskError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_TASK_ID_LEN
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(TaskError::InvalidTaskId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TaskId {
    type Error = TaskError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<TaskId> for String {
    fn from(value: TaskId) -> Self {
        value.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Première famille de tâches : déterministes, sans effet de bord.
///
/// Ce sont volontairement des calculs vérifiables, pas « un shell générique ».
/// Une tâche qui exige un accès réseau ou une écriture disque est refusée par
/// [`TaskSpec::validate`] : la famille ne doit pas pouvoir contourner ses
/// propres limites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Rend l'entrée telle quelle.
    Echo,
    /// Rend le BLAKE3 hexadécimal de l'entrée.
    Blake3,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Echo => "echo",
            Self::Blake3 => "blake3",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "echo" => Some(Self::Echo),
            "blake3" => Some(Self::Blake3),
            _ => None,
        }
    }
}

impl std::fmt::Display for TaskKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Ressources demandées par une tâche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRequirements {
    pub cpu_cores: u32,
    pub ram_bytes: u64,
    pub storage_bytes: u64,
    pub gpu_vram_bytes: u64,
}

impl TaskRequirements {
    fn validate(&self) -> Result<(), TaskError> {
        if self.cpu_cores == 0 || self.cpu_cores > MAX_TASK_CPU_CORES {
            return Err(TaskError::InvalidRequirement("cpu_cores hors bornes"));
        }
        if self.ram_bytes == 0 || self.ram_bytes > MAX_TASK_RESOURCE_BYTES {
            return Err(TaskError::InvalidRequirement("ram_bytes hors bornes"));
        }
        if self.storage_bytes > MAX_TASK_RESOURCE_BYTES {
            return Err(TaskError::InvalidRequirement("storage_bytes hors bornes"));
        }
        if self.gpu_vram_bytes > MAX_TASK_RESOURCE_BYTES {
            return Err(TaskError::InvalidRequirement("gpu_vram_bytes hors bornes"));
        }
        Ok(())
    }
}

/// Permissions accordées à l'exécution d'une tâche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPermissions {
    pub allow_network: bool,
    pub allow_filesystem_write: bool,
    pub max_output_bytes: u64,
}

impl Default for TaskPermissions {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_filesystem_write: false,
            max_output_bytes: 64 * 1024,
        }
    }
}

/// Contraintes d'exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskConstraints {
    /// Échéance absolue, en secondes Unix.
    pub deadline_unix: u64,
    /// Nombre total de tentatives autorisées (1 à 8).
    pub max_attempts: u32,
    pub permissions: TaskPermissions,
}

/// Spécification complète d'une tâche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: TaskId,
    pub owner: String,
    pub kind: TaskKind,
    #[serde(with = "hex_bytes")]
    pub input: Vec<u8>,
    pub requirements: TaskRequirements,
    pub constraints: TaskConstraints,
}

impl TaskSpec {
    /// Valide la spécification à l'instant `now` (secondes Unix).
    ///
    /// Toute la validation vit ici : une tâche ne peut pas être publiée avec
    /// une entrée hors bornes, une échéance absurde ou une permission que sa
    /// famille n'autorise pas.
    pub fn validate(&self, now: u64) -> Result<(), TaskError> {
        if self.owner.trim().is_empty() {
            return Err(TaskError::EmptyOwner);
        }
        if self.owner.len() > MAX_OWNER_LEN {
            return Err(TaskError::OwnerTooLong);
        }
        if self.owner.chars().any(char::is_control) {
            return Err(TaskError::OwnerControlCharacter);
        }
        if self.input.len() > MAX_TASK_INPUT_BYTES {
            return Err(TaskError::InputTooLarge {
                size: self.input.len(),
            });
        }
        self.requirements.validate()?;
        if self.constraints.deadline_unix <= now {
            return Err(TaskError::DeadlineInPast);
        }
        if self.constraints.deadline_unix > now.saturating_add(MAX_TASK_DEADLINE_HORIZON_SECS) {
            return Err(TaskError::DeadlineTooFar);
        }
        if self.constraints.max_attempts == 0 || self.constraints.max_attempts > MAX_TASK_ATTEMPTS {
            return Err(TaskError::InvalidConstraint("max_attempts hors bornes"));
        }
        let max_output = self.constraints.permissions.max_output_bytes;
        if max_output == 0 || max_output > MAX_TASK_OUTPUT_BYTES {
            return Err(TaskError::InvalidConstraint("max_output_bytes hors bornes"));
        }
        // Les deux tâches de la première famille sont déterministes : elles
        // n'ont aucune raison de parler au réseau ni d'écrire sur le disque.
        // Accepter ces permissions ici reviendrait à les promettre à un
        // exécuteur qui ne les applique pas encore.
        if self.constraints.permissions.allow_network {
            return Err(TaskError::UnsupportedPermission("allow_network"));
        }
        if self.constraints.permissions.allow_filesystem_write {
            return Err(TaskError::UnsupportedPermission("allow_filesystem_write"));
        }
        Ok(())
    }
}

/// État d'une tâche.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Created,
    Validated,
    Published,
    Assigned,
    Accepted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Validated => "validated",
            Self::Published => "published",
            Self::Assigned => "assigned",
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "created" => Some(Self::Created),
            "validated" => Some(Self::Validated),
            "published" => Some(Self::Published),
            "assigned" => Some(Self::Assigned),
            "accepted" => Some(Self::Accepted),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Expired
        )
    }

    /// Transitions autorisées, dans l'ordre du cycle de vie.
    ///
    /// Les états terminaux n'ont aucune sortie. `Expired` est accessible depuis
    /// tout état non terminal, mais seulement après l'échéance (vérifié par
    /// [`TaskRecord::expire`]).
    pub fn allowed_transitions(self) -> &'static [TaskState] {
        use TaskState::*;
        match self {
            Created => &[Validated, Cancelled, Expired],
            Validated => &[Published, Cancelled, Expired],
            Published => &[Assigned, Cancelled, Expired],
            Assigned => &[Accepted, Failed, Cancelled, Expired],
            Accepted => &[Running, Failed, Cancelled, Expired],
            Running => &[Succeeded, Failed, Cancelled, Expired],
            Succeeded | Failed | Cancelled | Expired => &[],
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        self.allowed_transitions().contains(&next)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Résultat vérifiable d'une tâche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskResult {
    #[serde(with = "hex_bytes")]
    pub output: Vec<u8>,
    /// BLAKE3 hexadécimal de `output`, calculé à la construction.
    pub output_hash: String,
    /// Nœud qui a produit le résultat.
    pub producer: String,
    /// Instant de production, en secondes Unix.
    pub produced_at: u64,
}

impl TaskResult {
    /// Construit un résultat et calcule son hash ; ne fait pas confiance à un
    /// hash fourni par l'appelant.
    pub fn new(
        output: Vec<u8>,
        producer: impl Into<String>,
        produced_at: u64,
    ) -> Result<Self, TaskError> {
        if output.len() as u64 > MAX_TASK_OUTPUT_BYTES {
            return Err(TaskError::ResultTooLarge { size: output.len() });
        }
        let producer = producer.into();
        if producer.trim().is_empty() || producer.len() > MAX_PRODUCER_LEN {
            return Err(TaskError::InvalidProducer(producer));
        }
        let output_hash = blake3::hash(&output).to_hex().to_string();
        Ok(Self {
            output,
            output_hash,
            producer,
            produced_at,
        })
    }

    /// Vérifie l'intégrité : hash recalculé, producteur non vide, taille bornée.
    pub fn verify(&self) -> Result<(), TaskError> {
        if self.output.len() as u64 > MAX_TASK_OUTPUT_BYTES {
            return Err(TaskError::ResultTooLarge {
                size: self.output.len(),
            });
        }
        if self.producer.trim().is_empty() || self.producer.len() > MAX_PRODUCER_LEN {
            return Err(TaskError::InvalidProducer(self.producer.clone()));
        }
        let computed = blake3::hash(&self.output).to_hex().to_string();
        if computed != self.output_hash {
            return Err(TaskError::ResultHashMismatch {
                expected: self.output_hash.clone(),
                computed,
            });
        }
        Ok(())
    }
}

/// Une tâche et son état courant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub spec: TaskSpec,
    pub state: TaskState,
    pub created_at: u64,
    pub updated_at: u64,
    pub attempts: u32,
    pub assigned_node: Option<String>,
    pub result: Option<TaskResult>,
    pub last_error: Option<String>,
}

impl TaskRecord {
    /// Crée une tâche à l'état `created`, après validation de sa spec.
    pub fn new(spec: TaskSpec, now: u64) -> Result<Self, TaskError> {
        spec.validate(now)?;
        Ok(Self {
            spec,
            state: TaskState::Created,
            created_at: now,
            updated_at: now,
            attempts: 0,
            assigned_node: None,
            result: None,
            last_error: None,
        })
    }

    fn advance(&mut self, next: TaskState, now: u64) -> Result<(), TaskError> {
        if self.state.is_terminal() {
            return Err(TaskError::TerminalState(self.state));
        }
        if !self.state.can_transition_to(next) {
            return Err(TaskError::IllegalTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.updated_at = now;
        Ok(())
    }

    pub fn mark_validated(&mut self, now: u64) -> Result<(), TaskError> {
        self.advance(TaskState::Validated, now)
    }

    pub fn publish(&mut self, now: u64) -> Result<(), TaskError> {
        self.advance(TaskState::Published, now)
    }

    /// Assigne la tâche à un nœud. Incrémente le compteur de tentatives et
    /// refuse au-delà de `max_attempts`.
    pub fn assign(&mut self, node: impl Into<String>, now: u64) -> Result<(), TaskError> {
        let node = node.into();
        if node.trim().is_empty() || node.len() > MAX_PRODUCER_LEN {
            return Err(TaskError::InvalidProducer(node));
        }
        if self.attempts >= self.spec.constraints.max_attempts {
            return Err(TaskError::AttemptsExhausted {
                attempts: self.attempts,
                max: self.spec.constraints.max_attempts,
            });
        }
        self.advance(TaskState::Assigned, now)?;
        self.attempts = self.attempts.saturating_add(1);
        self.assigned_node = Some(node);
        Ok(())
    }

    pub fn accept(&mut self, now: u64) -> Result<(), TaskError> {
        self.advance(TaskState::Accepted, now)
    }

    pub fn start(&mut self, now: u64) -> Result<(), TaskError> {
        self.advance(TaskState::Running, now)
    }

    /// Termine la tâche avec un résultat vérifié.
    ///
    /// Trois invariants : le résultat est intégre (hash recalculé), il vient du
    /// nœud assigné, et il n'est pas daté dans le futur.
    pub fn complete(&mut self, result: TaskResult, now: u64) -> Result<(), TaskError> {
        result.verify()?;
        if result.produced_at > now {
            return Err(TaskError::ResultFromFuture);
        }
        if result.produced_at < self.created_at {
            return Err(TaskError::ResultFromFuture);
        }
        let assigned = self
            .assigned_node
            .as_deref()
            .ok_or(TaskError::MissingAssignment)?;
        if result.producer != assigned {
            return Err(TaskError::ProducerMismatch {
                producer: result.producer,
                assigned: assigned.to_string(),
            });
        }
        self.advance(TaskState::Succeeded, now)?;
        self.result = Some(result);
        Ok(())
    }

    pub fn fail(&mut self, reason: impl Into<String>, now: u64) -> Result<(), TaskError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(TaskError::EmptyFailureReason);
        }
        self.advance(TaskState::Failed, now)?;
        self.last_error = Some(reason);
        Ok(())
    }

    pub fn cancel(&mut self, now: u64) -> Result<(), TaskError> {
        self.advance(TaskState::Cancelled, now)
    }

    /// Expire la tâche **seulement** si l'échéance est atteinte.
    pub fn expire(&mut self, now: u64) -> Result<(), TaskError> {
        if now < self.spec.constraints.deadline_unix {
            return Err(TaskError::DeadlineNotReached);
        }
        self.advance(TaskState::Expired, now)
    }

    pub fn is_expired_at(&self, now: u64) -> bool {
        !self.state.is_terminal() && now >= self.spec.constraints.deadline_unix
    }
}

/// Sérialisation hexadécimale des octets : un tableau JSON d'entiers serait
/// illisible et coûteux pour une entrée binaire.
mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let value = String::deserialize(deserializer)?;
        hex::decode(&value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn requirements() -> TaskRequirements {
        TaskRequirements {
            cpu_cores: 1,
            ram_bytes: 64 * 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
        }
    }

    fn spec() -> TaskSpec {
        TaskSpec {
            id: TaskId::parse("task_0123456789abcdef0123456789abcdef").unwrap(),
            owner: "owner-node".to_string(),
            kind: TaskKind::Echo,
            input: b"hello".to_vec(),
            requirements: requirements(),
            constraints: TaskConstraints {
                deadline_unix: NOW + 300,
                max_attempts: 2,
                permissions: TaskPermissions::default(),
            },
        }
    }

    fn running() -> TaskRecord {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        task.mark_validated(NOW).unwrap();
        task.publish(NOW).unwrap();
        task.assign("node-a", NOW).unwrap();
        task.accept(NOW).unwrap();
        task.start(NOW).unwrap();
        task
    }

    #[test]
    fn lifecycle_reaches_success_in_order() {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        assert_eq!(task.state, TaskState::Created);

        task.mark_validated(NOW + 1).unwrap();
        assert_eq!(task.state, TaskState::Validated);
        task.publish(NOW + 2).unwrap();
        assert_eq!(task.state, TaskState::Published);
        task.assign("node-a", NOW + 3).unwrap();
        assert_eq!(task.state, TaskState::Assigned);
        assert_eq!(task.attempts, 1);
        task.accept(NOW + 4).unwrap();
        task.start(NOW + 5).unwrap();
        assert_eq!(task.state, TaskState::Running);

        let result = TaskResult::new(b"hello".to_vec(), "node-a", NOW + 6).unwrap();
        task.complete(result, NOW + 7).unwrap();

        assert_eq!(task.state, TaskState::Succeeded);
        assert_eq!(task.updated_at, NOW + 7);
        assert!(task.result.as_ref().unwrap().verify().is_ok());
        assert!(task.state.is_terminal());
    }

    #[test]
    fn illegal_transition_names_both_states() {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        let error = task.start(NOW + 1).unwrap_err();
        assert_eq!(
            error,
            TaskError::IllegalTransition {
                from: TaskState::Created,
                to: TaskState::Running
            }
        );
        assert_eq!(task.state, TaskState::Created, "l'état n'a pas bougé");
    }

    #[test]
    fn terminal_state_is_immutable() {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        task.cancel(NOW + 1).unwrap();
        let error = task.publish(NOW + 2).unwrap_err();
        assert_eq!(error, TaskError::TerminalState(TaskState::Cancelled));
        assert_eq!(task.state, TaskState::Cancelled);
    }

    #[test]
    fn cancel_after_success_is_refused() {
        let mut task = running();
        let result = TaskResult::new(b"hello".to_vec(), "node-a", NOW + 6).unwrap();
        task.complete(result, NOW + 7).unwrap();
        let error = task.cancel(NOW + 8).unwrap_err();
        assert_eq!(error, TaskError::TerminalState(TaskState::Succeeded));
    }

    #[test]
    fn expire_requires_the_deadline_to_be_reached() {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        assert!(!task.is_expired_at(NOW + 299));
        assert!(task.is_expired_at(NOW + 300));
        assert_eq!(
            task.expire(NOW + 299).unwrap_err(),
            TaskError::DeadlineNotReached
        );
        task.expire(NOW + 300).unwrap();
        assert_eq!(task.state, TaskState::Expired);
        // Une tâche déjà terminale n'est plus « à expirer ».
        assert!(!task.is_expired_at(NOW + 301));
    }

    #[test]
    fn complete_rejects_a_tampered_result() {
        let mut task = running();
        let mut result = TaskResult::new(b"hello".to_vec(), "node-a", NOW + 6).unwrap();
        result.output = b"tampered".to_vec();

        let error = task.complete(result, NOW + 7).unwrap_err();
        assert!(matches!(error, TaskError::ResultHashMismatch { .. }));
        assert_eq!(
            task.state,
            TaskState::Running,
            "échec sans changement d'état"
        );
    }

    #[test]
    fn complete_rejects_a_result_from_another_node() {
        let mut task = running();
        let result = TaskResult::new(b"hello".to_vec(), "node-b", NOW + 6).unwrap();
        let error = task.complete(result, NOW + 7).unwrap_err();
        assert_eq!(
            error,
            TaskError::ProducerMismatch {
                producer: "node-b".to_string(),
                assigned: "node-a".to_string()
            }
        );
    }

    #[test]
    fn complete_rejects_a_result_from_the_future() {
        let mut task = running();
        let result = TaskResult::new(b"hello".to_vec(), "node-a", NOW + 50).unwrap();
        assert_eq!(
            task.complete(result, NOW + 10).unwrap_err(),
            TaskError::ResultFromFuture
        );
    }

    #[test]
    fn spec_rejects_values_out_of_bounds() {
        let mut invalid = spec();
        invalid.requirements.cpu_cores = 0;
        assert!(matches!(
            invalid.validate(NOW),
            Err(TaskError::InvalidRequirement(_))
        ));

        let mut invalid = spec();
        invalid.input = vec![0u8; MAX_TASK_INPUT_BYTES + 1];
        assert!(matches!(
            invalid.validate(NOW),
            Err(TaskError::InputTooLarge { .. })
        ));

        let mut invalid = spec();
        invalid.constraints.deadline_unix = NOW;
        assert_eq!(invalid.validate(NOW), Err(TaskError::DeadlineInPast));

        let mut invalid = spec();
        invalid.constraints.deadline_unix = NOW + MAX_TASK_DEADLINE_HORIZON_SECS + 1;
        assert_eq!(invalid.validate(NOW), Err(TaskError::DeadlineTooFar));

        let mut invalid = spec();
        invalid.constraints.max_attempts = 0;
        assert!(matches!(
            invalid.validate(NOW),
            Err(TaskError::InvalidConstraint(_))
        ));
    }

    #[test]
    fn deterministic_family_refuses_network_and_disk_permissions() {
        let mut network = spec();
        network.constraints.permissions.allow_network = true;
        assert_eq!(
            network.validate(NOW),
            Err(TaskError::UnsupportedPermission("allow_network"))
        );

        let mut disk = spec();
        disk.constraints.permissions.allow_filesystem_write = true;
        assert_eq!(
            disk.validate(NOW),
            Err(TaskError::UnsupportedPermission("allow_filesystem_write"))
        );
    }

    #[test]
    fn owner_must_be_non_empty_and_without_control_characters() {
        let mut empty = spec();
        empty.owner = "   ".to_string();
        assert_eq!(empty.validate(NOW), Err(TaskError::EmptyOwner));

        let mut control = spec();
        control.owner = "owner\nnode".to_string();
        assert_eq!(control.validate(NOW), Err(TaskError::OwnerControlCharacter));
    }

    #[test]
    fn task_id_is_bounded_and_alphanumeric() {
        assert!(TaskId::parse("task_abc-123").is_ok());
        for invalid in ["", "task/abc", "task abc", "é"] {
            assert!(
                TaskId::parse(invalid).is_err(),
                "{invalid:?} ne doit pas être un identifiant"
            );
        }
        let too_long = "a".repeat(MAX_TASK_ID_LEN + 1);
        assert!(TaskId::parse(too_long).is_err());
    }

    #[test]
    fn assignments_are_bounded_by_max_attempts() {
        let mut task = TaskRecord::new(spec(), NOW).unwrap();
        task.mark_validated(NOW).unwrap();
        task.publish(NOW).unwrap();
        task.assign("node-a", NOW).unwrap();
        task.fail("boom", NOW + 1).unwrap();

        // `failed` est terminal : on ne peut pas réassigner la même tâche.
        let error = task.assign("node-b", NOW + 2).unwrap_err();
        assert_eq!(error, TaskError::TerminalState(TaskState::Failed));

        // Une tâche publique peut être assignée jusqu'à `max_attempts`.
        let mut fresh = TaskRecord::new(spec(), NOW).unwrap();
        fresh.mark_validated(NOW).unwrap();
        fresh.publish(NOW).unwrap();
        fresh.assign("node-a", NOW).unwrap();
        fresh.assign("node-b", NOW + 1).unwrap_err();
        assert_eq!(fresh.attempts, 1);
    }

    #[test]
    fn json_uses_hex_for_binary_input_and_output() {
        let task = TaskRecord::new(spec(), NOW).unwrap();
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["spec"]["input"], "68656c6c6f");
        assert_eq!(json["state"], "created");

        let decoded: TaskRecord = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, task);
    }

    #[test]
    fn result_hash_is_computed_not_trusted() {
        let result = TaskResult::new(b"abc".to_vec(), "node-a", NOW).unwrap();
        assert_eq!(
            result.output_hash,
            blake3::hash(b"abc").to_hex().to_string()
        );

        let mut forged = result.clone();
        forged.output_hash = "00".repeat(32);
        assert!(matches!(
            forged.verify(),
            Err(TaskError::ResultHashMismatch { .. })
        ));
    }
}
