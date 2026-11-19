//! Magasin de tâches de la gateway : création, lecture, annulation, persistance.
//!
//! **Deux modes explicites.** Un magasin ouvert avec [`TaskStore::new`] reste
//! inerte : une tâche est validée puis publiée et y reste `published`. Un
//! magasin ouvert avec [`TaskStore::with_executor`] attache un exécuteur local :
//! il assigne, exécute les familles déterministes `echo`/`blake3`, signe le
//! résultat avec sa clé ML-DSA-65, vérifie hash et signature, puis persiste
//! l'état final. Aucune route HTTP ne fabrique un résultat : elle lit celui que
//! l'exécuteur a produit.
//!
//! La persistance est atomique (temporaire, `sync_all`, `rename`, fsync du
//! répertoire, `0600`). Un échec d'écriture annule la création : une tâche
//! n'est jamais reconnue si elle n'est pas durable. Un fichier illisible est
//! mis en quarantaine, jamais supprimé.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polygone_core::sign::{
    KeyPair, PublicKey, SecretKey, Signer, Verifier, generate_keypair, node_id_from_sign_pk_hex,
};
use polygone_core::task::{
    MAX_TASK_DEADLINE_HORIZON_SECS, TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRecord,
    TaskRequirements, TaskResult, TaskSpec, TaskState,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Nombre maximal de tâches conservées.
pub const MAX_TASKS: usize = 4096;
/// Taille de page par défaut.
pub const DEFAULT_TASK_LIST_LIMIT: usize = 50;
/// Taille de page maximale.
pub const MAX_TASK_LIST_LIMIT: usize = 200;

fn default_cpu_cores() -> u32 {
    1
}

fn default_ram_bytes() -> u64 {
    64 * 1024 * 1024
}

fn default_deadline_seconds() -> u64 {
    300
}

fn default_max_attempts() -> u32 {
    1
}

fn default_max_output_bytes() -> u64 {
    64 * 1024
}

/// Corps de `POST /api/v1/tasks`.
///
/// `deny_unknown_fields` est volontaire : un client ne peut pas glisser un
/// `state`, un `id` ou un `result` que le serveur prendrait pour argent
/// comptant.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTaskRequest {
    pub owner: String,
    pub kind: String,
    #[serde(default)]
    pub input_hex: String,
    #[serde(default = "default_cpu_cores")]
    pub cpu_cores: u32,
    #[serde(default = "default_ram_bytes")]
    pub ram_bytes: u64,
    #[serde(default)]
    pub storage_bytes: u64,
    #[serde(default)]
    pub gpu_vram_bytes: u64,
    #[serde(default = "default_deadline_seconds")]
    pub deadline_in_seconds: u64,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default)]
    pub allow_filesystem_write: bool,
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: u64,
}

impl CreateTaskRequest {
    fn into_spec(self, id: TaskId, now: u64) -> Result<TaskSpec, StoreError> {
        let kind = TaskKind::parse(&self.kind)
            .ok_or_else(|| StoreError::Invalid(format!("type de tâche inconnu : {}", self.kind)))?;
        let input = hex::decode(&self.input_hex)
            .map_err(|_| StoreError::Invalid("input_hex n'est pas de l'hexadécimal".into()))?;
        if self.deadline_in_seconds == 0
            || self.deadline_in_seconds > MAX_TASK_DEADLINE_HORIZON_SECS
        {
            return Err(StoreError::Invalid(
                "deadline_in_seconds hors bornes".into(),
            ));
        }
        Ok(TaskSpec {
            id,
            owner: self.owner,
            kind,
            input,
            requirements: TaskRequirements {
                cpu_cores: self.cpu_cores,
                ram_bytes: self.ram_bytes,
                storage_bytes: self.storage_bytes,
                gpu_vram_bytes: self.gpu_vram_bytes,
            },
            constraints: TaskConstraints {
                deadline_unix: now.saturating_add(self.deadline_in_seconds),
                max_attempts: self.max_attempts,
                permissions: TaskPermissions {
                    allow_network: self.allow_network,
                    allow_filesystem_write: self.allow_filesystem_write,
                    max_output_bytes: self.max_output_bytes,
                },
            },
        })
    }
}

/// Erreurs du magasin, traduites en codes HTTP par le routeur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    Invalid(String),
    NotFound,
    Conflict(String),
    Full,
    Persistence(String),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "{message}"),
            Self::NotFound => formatter.write_str("tâche inconnue"),
            Self::Conflict(message) => write!(formatter, "{message}"),
            Self::Full => formatter.write_str("limite de tâches atteinte"),
            Self::Persistence(message) => write!(formatter, "persistance impossible : {message}"),
        }
    }
}

/// Page de tâches renvoyée par `GET /api/v1/tasks`.
#[derive(Debug, Clone, Serialize)]
pub struct TaskPage {
    pub tasks: Vec<TaskRecord>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
    /// Vrai si ce magasin exécute réellement les tâches qu'il publie. Reste
    /// faux pour un magasin de test ou de lecture seule, afin qu'une interface
    /// ne présente jamais une file comme un calcul en cours.
    pub executor_attached: bool,
}

#[derive(Clone)]
struct TaskExecutor {
    /// `NodeId` dérivé de `keypair.verifier` : le producteur signé ne peut pas
    /// être un simple label choisi par l'appelant.
    node: String,
    keypair: KeyPair,
}

struct Inner {
    tasks: HashMap<String, TaskRecord>,
    file: Option<PathBuf>,
    /// Exécuteur local, ou `None` pour un magasin inerte.
    executor: Option<TaskExecutor>,
}

/// Magasin partageable (clones partagent le même état).
#[derive(Clone)]
pub struct TaskStore {
    inner: Arc<Mutex<Inner>>,
    /// Bus d'événements optionnel (F1-EVT-01) : chaque transition réelle
    /// (création, annulation, reprise, expiration) y est publiée. `None` =
    /// magasin de test silencieux, jamais un bus fantôme.
    events: Option<crate::events::EventBus>,
}

impl TaskStore {
    /// Ouvre un magasin **inerte** ; sans fichier, l'état est en mémoire
    /// seulement. Les tâches y restent `published` : aucun exécuteur.
    pub fn new(file: Option<PathBuf>) -> Self {
        Self::open(file, None)
    }

    /// Ouvre un magasin qui exécute réellement les tâches publiées et signe
    /// leurs résultats avec `keypair`. Le `NodeId` du producteur est dérivé de
    /// la clé publique, jamais fourni comme un label libre. Les tâches laissées
    /// dans un état intermédiaire par un arrêt brutal sont reprises par
    /// [`TaskStore::recover`] avec la même clé.
    pub fn with_executor(file: Option<PathBuf>, keypair: KeyPair) -> Self {
        let node = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
            .expect("une clé ML-DSA-65 générée dérive toujours un NodeId");
        Self::open(file, Some(TaskExecutor { node, keypair }))
    }

    fn open(file: Option<PathBuf>, executor: Option<TaskExecutor>) -> Self {
        let tasks = match file.as_deref() {
            Some(path) => load(path),
            None => HashMap::new(),
        };
        Self {
            inner: Arc::new(Mutex::new(Inner {
                tasks,
                file,
                executor,
            })),
            events: None,
        }
    }

    /// Attache le bus d'événements de la gateway. Les clones du magasin
    /// partagent le même bus : une transition n'est publiée qu'une fois.
    pub fn with_events(mut self, events: crate::events::EventBus) -> Self {
        self.events = Some(events);
        self
    }

    fn emit(&self, kind: &str, data: serde_json::Value) {
        if let Some(bus) = &self.events {
            bus.emit(kind, data);
        }
    }

    /// Balaye les échéances comme `sweep_expired`, en publiant un événement
    /// `task_expired` par tâche réellement expirée. L'ordre des ids suit la
    /// table ; le bus réattribue ses propres ids monotones.
    fn sweep_and_emit(&self, inner: &mut Inner, now: u64) -> bool {
        let expiring: Vec<String> = inner
            .tasks
            .iter()
            .filter(|(_, task)| task.is_expired_at(now))
            .map(|(id, _)| id.clone())
            .collect();
        let swept = sweep_expired(&mut inner.tasks, now);
        for id in expiring {
            if inner
                .tasks
                .get(&id)
                .is_some_and(|task| task.state == TaskState::Expired)
            {
                self.emit(
                    "task_expired",
                    serde_json::json!({ "id": id, "state": "expired" }),
                );
            }
        }
        swept
    }

    /// Vrai si ce magasin exécute les tâches au lieu de seulement les publier.
    pub async fn executor_attached(&self) -> bool {
        self.inner.lock().await.executor.is_some()
    }

    /// Vrai si les tâches survivent au redémarrage. Un magasin en mémoire
    /// seulement ne le prétend pas.
    pub async fn persisted(&self) -> bool {
        self.inner.lock().await.file.is_some()
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.tasks.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Répartition des tâches par état. **Tous** les états sont présents, même
    /// à zéro : une omission se lirait comme « état inconnu » plutôt que comme
    /// « aucune tâche ». Les tâches expirées sont balayées avant le comptage.
    pub async fn counts_by_state(&self, now: u64) -> BTreeMap<String, usize> {
        let mut inner = self.inner.lock().await;
        let swept = self.sweep_and_emit(&mut inner, now);
        let mut counts: BTreeMap<String, usize> = TaskState::ALL
            .into_iter()
            .map(|state| (state.as_str().to_string(), 0))
            .collect();
        for task in inner.tasks.values() {
            *counts.entry(task.state.as_str().to_string()).or_insert(0) += 1;
        }
        if swept {
            persist_locked(&inner);
        }
        counts
    }

    /// Crée une tâche validée et publiée, durablement si un fichier est
    /// configuré.
    pub async fn create(
        &self,
        request: CreateTaskRequest,
        now: u64,
    ) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        let swept = self.sweep_and_emit(&mut inner, now);
        if inner.tasks.len() >= MAX_TASKS {
            if swept {
                persist_locked(&inner);
            }
            return Err(StoreError::Full);
        }

        let id = loop {
            let candidate = TaskId::parse(format!("task_{}", crate::uuid_short()))
                .map_err(|error| StoreError::Invalid(error.to_string()))?;
            if !inner.tasks.contains_key(candidate.as_str()) {
                break candidate;
            }
        };
        let spec = request.into_spec(id, now)?;
        let mut task =
            TaskRecord::new(spec, now).map_err(|error| StoreError::Invalid(error.to_string()))?;
        task.mark_validated(now)
            .and_then(|_| task.publish(now))
            .map_err(|error| StoreError::Invalid(error.to_string()))?;

        let id = task.spec.id.as_str().to_string();
        inner.tasks.insert(id.clone(), task.clone());

        // L'exécuteur est local et déterministe : la tâche quitte `published`
        // dans le même appel. Si l'écriture finale échoue, elle est retirée
        // comme toute création non durable.
        if let Some(executor) = inner.executor.clone() {
            execute_record(&mut task, &executor, now);
            inner.tasks.insert(id.clone(), task.clone());
        }

        if let Some(path) = inner.file.clone()
            && let Err(error) = persist(&path, &inner.tasks)
        {
            inner.tasks.remove(&id);
            return Err(StoreError::Persistence(error));
        }
        self.emit(
            "task_created",
            serde_json::json!({ "id": task.spec.id.as_str(), "state": task.state.as_str() }),
        );
        Ok(task)
    }

    pub async fn get(&self, id: &str, now: u64) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        let swept = self.sweep_and_emit(&mut inner, now);
        let task = inner.tasks.get(id).cloned().ok_or(StoreError::NotFound)?;
        if swept {
            persist_locked(&inner);
        }
        Ok(task)
    }

    /// Liste filtrée, triée du plus récent au plus ancien, paginée.
    pub async fn list(
        &self,
        state: Option<TaskState>,
        limit: usize,
        offset: usize,
        now: u64,
    ) -> Result<TaskPage, StoreError> {
        if limit == 0 || limit > MAX_TASK_LIST_LIMIT {
            return Err(StoreError::Invalid("limit hors bornes".into()));
        }
        let mut inner = self.inner.lock().await;
        let swept = self.sweep_and_emit(&mut inner, now);

        let mut tasks: Vec<TaskRecord> = inner
            .tasks
            .values()
            .filter(|task| state.is_none_or(|state| task.state == state))
            .cloned()
            .collect();
        tasks.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.spec.id.as_str().cmp(right.spec.id.as_str()))
        });

        let total = tasks.len();
        let tasks = tasks.into_iter().skip(offset).take(limit).collect();
        if swept {
            persist_locked(&inner);
        }
        Ok(TaskPage {
            tasks,
            total,
            limit,
            offset,
            executor_attached: inner.executor.is_some(),
        })
    }

    /// Annule une tâche non terminale. L'état n'est reconnu qu'après écriture
    /// durable : un échec de persistance laisse la tâche inchangée.
    pub async fn cancel(&self, id: &str, now: u64) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        self.sweep_and_emit(&mut inner, now);

        let mut task = inner.tasks.get(id).cloned().ok_or(StoreError::NotFound)?;
        task.cancel(now)
            .map_err(|error| StoreError::Conflict(error.to_string()))?;

        if let Some(path) = inner.file.clone() {
            let mut snapshot = inner.tasks.clone();
            snapshot.insert(id.to_string(), task.clone());
            if let Err(error) = persist(&path, &snapshot) {
                return Err(StoreError::Persistence(error));
            }
        }
        inner.tasks.insert(id.to_string(), task.clone());
        self.emit(
            "task_cancelled",
            serde_json::json!({ "id": id, "state": task.state.as_str() }),
        );
        Ok(task)
    }

    /// Reprend les tâches laissées dans un état non terminal par un arrêt
    /// brutal : les publie ou reprend leur cycle, puis les exécute. Sans
    /// exécuteur, ne fait rien et rend `0`.
    pub async fn recover(&self, now: u64) -> usize {
        let mut inner = self.inner.lock().await;
        let Some(executor) = inner.executor.clone() else {
            return 0;
        };
        let swept = self.sweep_and_emit(&mut inner, now);
        let mut ids: Vec<String> = inner
            .tasks
            .iter()
            .filter(|(_, task)| {
                matches!(
                    task.state,
                    TaskState::Published
                        | TaskState::Assigned
                        | TaskState::Accepted
                        | TaskState::Running
                )
            })
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();

        let mut changed = 0;
        for id in ids {
            let Some(task) = inner.tasks.get_mut(&id) else {
                continue;
            };
            let before = task.state;
            execute_record(task, &executor, now);
            if task.state != before {
                changed += 1;
                self.emit(
                    "task_resumed",
                    serde_json::json!({ "id": id, "state": task.state.as_str() }),
                );
            }
        }
        if swept || changed > 0 {
            persist_locked(&inner);
        }
        changed
    }
}

/// Fait avancer une tâche jusqu'à `succeeded` ou `failed`.
///
/// Le producteur est le nœud assigné s'il existe déjà (reprise d'une tâche
/// interrompue), sinon le nœud qui exécute. Un échec de calcul devient un état
/// `failed` explicite, jamais une panique ni une tâche bloquée en `running`.
fn execute_record(task: &mut TaskRecord, executor: &TaskExecutor, now: u64) {
    let transition = match task.state {
        TaskState::Published => task
            .assign(executor.node.clone(), now)
            .and_then(|_| task.accept(now))
            .and_then(|_| task.start(now)),
        TaskState::Assigned => task.accept(now).and_then(|_| task.start(now)),
        TaskState::Accepted => task.start(now),
        TaskState::Running => Ok(()),
        _ => return,
    };
    if let Err(error) = transition {
        let _ = task.fail(error.to_string(), now);
        return;
    }

    // Une tâche reprise peut avoir été assignée à une autre identité. On ne
    // signe jamais sous un nom qui n'est pas le nôtre : l'échec est explicite.
    let producer = task
        .assigned_node
        .clone()
        .unwrap_or_else(|| executor.node.clone());
    if producer != executor.node {
        let _ = task.fail(
            format!(
                "nœud assigné {producer} différent de l'identité de l'exécuteur {}",
                executor.node
            ),
            now,
        );
        return;
    }

    let max_output = task.spec.constraints.permissions.max_output_bytes;
    let prepared = task
        .spec
        .kind
        .execute(&task.spec.input, max_output)
        .and_then(|output| {
            TaskResult::signed(
                task.spec.id.clone(),
                output,
                producer,
                now,
                &executor.keypair,
            )
        });

    match prepared {
        Ok(result) => {
            if let Err(error) = task.complete(result, now) {
                let _ = task.fail(error.to_string(), now);
            }
        }
        Err(error) => {
            let _ = task.fail(error.to_string(), now);
        }
    }
}

/// Fait expirer les tâches dont l'échéance est atteinte. Renvoie `true` si au
/// moins une tâche a changé d'état.
fn sweep_expired(tasks: &mut HashMap<String, TaskRecord>, now: u64) -> bool {
    let mut changed = false;
    for task in tasks.values_mut() {
        if task.is_expired_at(now) && task.expire(now).is_ok() {
            changed = true;
        }
    }
    changed
}

fn persist_locked(inner: &Inner) {
    if let Some(path) = inner.file.clone()
        && let Err(error) = persist(&path, &inner.tasks)
    {
        log::error!("gateway : persistance des tâches impossible : {error}");
    }
}

fn load(path: &Path) -> HashMap<String, TaskRecord> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            log::warn!(
                "gateway : lecture de {} impossible ({error}) — aucune tâche chargée",
                path.display()
            );
            return HashMap::new();
        }
    };
    let parsed: HashMap<String, TaskRecord> = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            let message = format!("fichier de tâches illisible : {error}");
            log::error!("gateway : {} — mise en quarantaine", path.display());
            match quarantine(path) {
                Some(target) => {
                    log::warn!("gateway : {message}; conservé sous {}", target.display())
                }
                None => log::error!("gateway : quarantaine impossible, fichier laissé en place"),
            }
            return HashMap::new();
        }
    };

    let mut tasks = HashMap::new();
    for (key, task) in parsed {
        if key != task.spec.id.as_str() {
            log::warn!("gateway : tâche {key} ignorée : identifiant incohérent");
            continue;
        }
        tasks.insert(key, task);
    }
    tasks
}

/// Déplace un fichier illisible à côté de l'original, sans jamais le supprimer.
fn quarantine(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().to_string();
    let target = path.with_file_name(format!(
        "{name}.corrupt-{}-{}",
        crate::now_secs(),
        std::process::id()
    ));
    std::fs::rename(path, &target).ok().map(|_| target)
}

/// Écrit le fichier de tâches de façon atomique et lisible par son seul
/// propriétaire.
fn persist(path: &Path, tasks: &HashMap<String, TaskRecord>) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(tasks).map_err(|error| error.to_string())?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(&body)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                std::fs::File::open(parent)?.sync_all()?;
            }
        }
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

/// Nom du fichier de clé de l'exécuteur, à côté du fichier de tâches.
pub const EXECUTOR_KEY_FILE: &str = "gateway-signing.json";

#[derive(Serialize, Deserialize)]
struct StoredExecutorKey {
    sign_pk_hex: String,
    sign_sk_hex: String,
}

/// Chemin de la clé de signature de l'exécuteur pour un fichier de tâches
/// donné. La clé vit dans le même répertoire, jamais dans un chemin implicite
/// distinct de l'état qu'elle signe.
pub fn executor_key_path(tasks_file: &Path) -> PathBuf {
    tasks_file.with_file_name(EXECUTOR_KEY_FILE)
}

/// Charge la paire de clés de l'exécuteur, ou la crée et la persiste si elle
/// n'existe pas encore. Une paire incohérente est refusée : mieux vaut ne pas
/// exécuter que signer avec une clé dont la publique ne correspond pas.
pub fn load_or_create_executor_keypair(tasks_file: &Path) -> Result<KeyPair, String> {
    let path = executor_key_path(tasks_file);
    match std::fs::read(&path) {
        Ok(bytes) => parse_executor_key(&bytes, &path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let keypair = generate_keypair().map_err(|error| error.to_string())?;
            persist_executor_key(&path, &keypair)?;
            Ok(keypair)
        }
        Err(error) => Err(format!(
            "lecture de {} impossible : {error}",
            path.display()
        )),
    }
}

fn parse_executor_key(bytes: &[u8], path: &Path) -> Result<KeyPair, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("métadonnées de {} illisibles : {error}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "clé d'exécuteur non régulière ou lien symbolique refusé : {}",
                path.display()
            ));
        }
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(format!(
                "permissions trop larges pour la clé d'exécuteur {} : {mode:03o}",
                path.display()
            ));
        }
    }

    let stored: StoredExecutorKey = serde_json::from_slice(bytes)
        .map_err(|error| format!("clé d'exécuteur illisible : {error}"))?;
    let public = PublicKey::from_hex(&stored.sign_pk_hex)
        .map_err(|_| "clé publique ML-DSA-65 invalide".to_string())?;
    let secret = SecretKey::from_hex(&stored.sign_sk_hex)
        .map_err(|_| "clé secrète ML-DSA-65 invalide".to_string())?;
    let keypair = KeyPair {
        signer: Signer::from_secret(secret),
        verifier: Verifier::from_public(public),
    };
    let probe = b"polygone-gateway-executor-key-probe-v1";
    if !keypair.verifier.verify(probe, &keypair.signer.sign(probe)) {
        return Err("paire de clés d'exécuteur incohérente".to_string());
    }
    Ok(keypair)
}

fn persist_executor_key(path: &Path, keypair: &KeyPair) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(&StoredExecutorKey {
        sign_pk_hex: keypair.verifier.public_key().to_hex(),
        sign_sk_hex: keypair.signer.secret_key().to_hex(),
    })
    .map_err(|error| error.to_string())?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    let result = (|| -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&body)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn request() -> CreateTaskRequest {
        CreateTaskRequest {
            owner: "owner-node".into(),
            kind: "echo".into(),
            input_hex: "68656c6c6f".into(),
            cpu_cores: 1,
            ram_bytes: 64 * 1024 * 1024,
            storage_bytes: 0,
            gpu_vram_bytes: 0,
            deadline_in_seconds: 300,
            max_attempts: 1,
            allow_network: false,
            allow_filesystem_write: false,
            max_output_bytes: 64 * 1024,
        }
    }

    fn executor_keypair() -> KeyPair {
        generate_keypair().unwrap()
    }

    fn executor_node(keypair: &KeyPair) -> String {
        node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap()
    }

    fn temp_path(label: &str) -> (PathBuf, PathBuf) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "polygone-gateway-tasks-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tasks.json");
        (dir, path)
    }

    #[tokio::test]
    async fn create_validates_then_publishes() {
        let store = TaskStore::new(None);
        let task = store.create(request(), NOW).await.unwrap();

        assert_eq!(task.state, TaskState::Published);
        assert!(task.spec.id.as_str().starts_with("task_"));
        assert_eq!(task.spec.input, b"hello");
        assert_eq!(task.spec.kind, TaskKind::Echo);
        assert_eq!(task.attempts, 0);
        assert_eq!(task.created_at, NOW);
        assert!(task.result.is_none());
        assert_eq!(store.len().await, 1);
    }

    #[tokio::test]
    async fn executor_completes_an_echo_task_with_a_verified_result() {
        let keypair = executor_keypair();
        let node = executor_node(&keypair);
        let public_key = keypair.verifier.public_key().to_hex();
        let store = TaskStore::with_executor(None, keypair);
        assert!(store.executor_attached().await);

        let task = store.create(request(), NOW).await.unwrap();
        assert_eq!(task.state, TaskState::Succeeded);
        assert_eq!(task.attempts, 1);
        assert_eq!(task.assigned_node.as_deref(), Some(node.as_str()));

        let result = task.result.expect("un résultat vérifié");
        assert_eq!(result.task_id, task.spec.id);
        assert_eq!(result.output, b"hello");
        assert_eq!(result.producer, node);
        assert_eq!(result.producer_public_key_hex, public_key);
        assert_eq!(
            result.signature_hex.len(),
            polygone_core::sign::SIGNATURE_SIZE * 2
        );
        result
            .verify()
            .expect("hash, nœud et signature doivent être recalculés");
    }

    #[tokio::test]
    async fn executor_computes_blake3_and_respects_the_output_limit() {
        let store = TaskStore::with_executor(None, executor_keypair());
        let mut hash = request();
        hash.kind = "blake3".into();
        hash.input_hex = "616263".into();
        let task = store.create(hash, NOW).await.unwrap();
        assert_eq!(task.state, TaskState::Succeeded);
        let output = task.result.unwrap().output;
        assert_eq!(output.len(), 64, "BLAKE3 hexadécimal : {output:?}");
        assert!(output.iter().all(u8::is_ascii_hexdigit));

        let mut too_small = request();
        too_small.max_output_bytes = 2;
        let failed = store.create(too_small, NOW + 1).await.unwrap();
        assert_eq!(failed.state, TaskState::Failed);
        assert!(failed.result.is_none());
        assert!(
            failed
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("trop grande")),
            "l'échec doit nommer la borne : {:?}",
            failed.last_error
        );
    }

    #[tokio::test]
    async fn executor_recovers_an_interrupted_task() {
        let (dir, path) = temp_path("recovery");
        let keypair = executor_keypair();
        let node = executor_node(&keypair);
        let interrupted = {
            let store = TaskStore::new(Some(path.clone()));
            let task = store.create(request(), NOW).await.unwrap();
            let mut interrupted = task.clone();
            interrupted.assign(node.clone(), NOW + 1).unwrap();
            let mut on_disk = HashMap::new();
            on_disk.insert(interrupted.spec.id.as_str().to_string(), interrupted);
            persist(&path, &on_disk).unwrap();
            task
        };

        let store = TaskStore::with_executor(Some(path.clone()), keypair);
        assert_eq!(store.recover(NOW + 2).await, 1);
        let recovered = store
            .get(interrupted.spec.id.as_str(), NOW + 2)
            .await
            .unwrap();
        assert_eq!(recovered.state, TaskState::Succeeded);
        assert_eq!(recovered.assigned_node.as_deref(), Some(node.as_str()));
        let result = recovered.result.unwrap();
        assert_eq!(result.producer, node);
        result
            .verify()
            .expect("la reprise signe avec la même identité");
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn recovery_refuses_to_sign_for_another_identity() {
        let (dir, path) = temp_path("recovery-foreign");
        let original = executor_keypair();
        let node = executor_node(&original);
        let interrupted = {
            let store = TaskStore::new(Some(path.clone()));
            let task = store.create(request(), NOW).await.unwrap();
            let mut interrupted = task.clone();
            interrupted.assign(node, NOW + 1).unwrap();
            let mut on_disk = HashMap::new();
            on_disk.insert(interrupted.spec.id.as_str().to_string(), interrupted);
            persist(&path, &on_disk).unwrap();
            task
        };

        let store = TaskStore::with_executor(Some(path.clone()), executor_keypair());
        assert_eq!(store.recover(NOW + 2).await, 1);
        let recovered = store
            .get(interrupted.spec.id.as_str(), NOW + 2)
            .await
            .unwrap();
        assert_eq!(recovered.state, TaskState::Failed);
        assert!(recovered.result.is_none());
        assert!(
            recovered
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("différent")),
            "l'échec doit nommer l'identité : {:?}",
            recovered.last_error
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn executor_key_is_created_private_and_reloaded() {
        let (dir, path) = temp_path("signing-key");
        let first = load_or_create_executor_keypair(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let key_path = executor_key_path(&path);
            assert_eq!(
                std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let reloaded = load_or_create_executor_keypair(&path).unwrap();
        assert_eq!(
            first.verifier.public_key().to_hex(),
            reloaded.verifier.public_key().to_hex(),
            "la clé doit survivre au redémarrage"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn create_refuses_unknown_kind_and_unsupported_permissions() {
        let store = TaskStore::new(None);

        let mut unknown = request();
        unknown.kind = "shell".into();
        assert!(matches!(
            store.create(unknown, NOW).await,
            Err(StoreError::Invalid(_))
        ));

        let mut network = request();
        network.allow_network = true;
        assert!(matches!(
            store.create(network, NOW).await,
            Err(StoreError::Invalid(_))
        ));

        let mut bad_hex = request();
        bad_hex.input_hex = "zz".into();
        assert!(matches!(
            store.create(bad_hex, NOW).await,
            Err(StoreError::Invalid(_))
        ));
        assert!(store.is_empty().await, "aucun refus ne crée de tâche");
    }

    #[tokio::test]
    async fn a_created_task_survives_a_restart() {
        let (dir, path) = temp_path("restart");
        let task = {
            let store = TaskStore::new(Some(path.clone()));
            store.create(request(), NOW).await.unwrap()
        };
        assert!(path.is_file());

        let reopened = TaskStore::new(Some(path.clone()));
        let reloaded = reopened.get(task.spec.id.as_str(), NOW + 1).await.unwrap();
        assert_eq!(reloaded, task);
        assert_eq!(reopened.len().await, 1);
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn cancel_is_durable_and_terminal() {
        let (dir, path) = temp_path("cancel");
        let store = TaskStore::new(Some(path.clone()));
        let task = store.create(request(), NOW).await.unwrap();
        let cancelled = store.cancel(task.spec.id.as_str(), NOW + 1).await.unwrap();
        assert_eq!(cancelled.state, TaskState::Cancelled);
        assert_eq!(cancelled.updated_at, NOW + 1);

        let second = store.cancel(task.spec.id.as_str(), NOW + 2).await;
        assert!(matches!(second, Err(StoreError::Conflict(_))));

        let reopened = TaskStore::new(Some(path.clone()));
        assert_eq!(
            reopened
                .get(task.spec.id.as_str(), NOW + 3)
                .await
                .unwrap()
                .state,
            TaskState::Cancelled
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn expired_tasks_are_swept_on_read() {
        let (dir, path) = temp_path("expiry");
        let store = TaskStore::new(Some(path.clone()));
        let mut request = request();
        request.deadline_in_seconds = 60;
        let task = store.create(request, NOW).await.unwrap();

        let page = store.list(None, 50, 0, NOW + 61).await.unwrap();
        assert_eq!(page.tasks[0].state, TaskState::Expired);
        assert_eq!(page.tasks[0].spec.id, task.spec.id);

        let reopened = TaskStore::new(Some(path.clone()));
        assert_eq!(
            reopened
                .get(task.spec.id.as_str(), NOW + 62)
                .await
                .unwrap()
                .state,
            TaskState::Expired
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn list_filters_and_paginates_deterministically() {
        let store = TaskStore::new(None);
        let first = store.create(request(), NOW).await.unwrap();
        let second = store.create(request(), NOW + 1).await.unwrap();
        let third = store.create(request(), NOW + 2).await.unwrap();
        store
            .cancel(second.spec.id.as_str(), NOW + 3)
            .await
            .unwrap();

        let page = store.list(None, 2, 0, NOW + 4).await.unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.limit, 2);
        assert_eq!(page.tasks.len(), 2);
        assert_eq!(page.tasks[0].spec.id, third.spec.id);
        assert_eq!(page.tasks[1].spec.id, second.spec.id);
        assert!(!page.executor_attached);

        let rest = store.list(None, 2, 2, NOW + 4).await.unwrap();
        assert_eq!(rest.tasks.len(), 1);
        assert_eq!(rest.tasks[0].spec.id, first.spec.id);

        let cancelled = store
            .list(Some(TaskState::Cancelled), 50, 0, NOW + 4)
            .await
            .unwrap();
        assert_eq!(cancelled.total, 1);
        assert_eq!(cancelled.tasks[0].spec.id, second.spec.id);

        assert!(matches!(
            store.list(None, 0, 0, NOW + 4).await,
            Err(StoreError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn a_corrupt_file_is_quarantined_not_deleted() {
        let (dir, path) = temp_path("corrupt");
        std::fs::write(&path, b"{pas du json").unwrap();

        let store = TaskStore::new(Some(path.clone()));
        assert!(store.is_empty().await);
        assert!(!path.exists(), "l'original illisible a été déplacé");

        let quarantined: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".corrupt-"))
            .collect();
        assert_eq!(
            quarantined.len(),
            1,
            "aucun fichier perdu : {quarantined:?}"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[tokio::test]
    async fn a_persistence_failure_does_not_acknowledge_the_task() {
        let (dir, _unused) = temp_path("persistence-failure");
        let blocker = dir.join("blocked");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let store = TaskStore::new(Some(blocker.join("tasks.json")));

        let result = store.create(request(), NOW).await;
        assert!(matches!(result, Err(StoreError::Persistence(_))));
        assert!(
            store.is_empty().await,
            "une tâche non durable ne doit pas exister en mémoire"
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
