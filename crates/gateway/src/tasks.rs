//! Magasin de tâches de la gateway : création, lecture, annulation, persistance.
//!
//! **Ce que ce magasin ne fait pas** : exécuter. Une tâche créée ici est
//! validée puis publiée ; elle reste `published` tant qu'aucun exécuteur ne
//! l'assigne. L'API ne peut donc jamais la marquer `succeeded` : le résultat
//! exige un nœud assigné et un hash vérifié, deux choses qu'aucune route HTTP
//! ne fabrique.
//!
//! La persistance est atomique (temporaire, `sync_all`, `rename`, fsync du
//! répertoire, `0600`). Un échec d'écriture annule la création : une tâche
//! n'est jamais reconnue si elle n'est pas durable. Un fichier illisible est
//! mis en quarantaine, jamais supprimé.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polygone_core::task::{
    MAX_TASK_DEADLINE_HORIZON_SECS, TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRecord,
    TaskRequirements, TaskSpec, TaskState,
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
    /// Toujours `false` dans cet incrément : aucun exécuteur n'est branché.
    /// Ce champ existe pour que l'interface ne présente pas une file comme un
    /// calcul en cours.
    pub executor_attached: bool,
}

struct Inner {
    tasks: HashMap<String, TaskRecord>,
    file: Option<PathBuf>,
}

/// Magasin partageable (clones partagent le même état).
#[derive(Clone)]
pub struct TaskStore {
    inner: Arc<Mutex<Inner>>,
}

impl TaskStore {
    /// Ouvre un magasin ; sans fichier, l'état est en mémoire seulement.
    pub fn new(file: Option<PathBuf>) -> Self {
        let tasks = match file.as_deref() {
            Some(path) => load(path),
            None => HashMap::new(),
        };
        Self {
            inner: Arc::new(Mutex::new(Inner { tasks, file })),
        }
    }

    pub async fn len(&self) -> usize {
        self.inner.lock().await.tasks.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Crée une tâche validée et publiée, durablement si un fichier est
    /// configuré.
    pub async fn create(
        &self,
        request: CreateTaskRequest,
        now: u64,
    ) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        let swept = sweep_expired(&mut inner.tasks, now);
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

        inner
            .tasks
            .insert(task.spec.id.as_str().to_string(), task.clone());

        if let Some(path) = inner.file.clone()
            && let Err(error) = persist(&path, &inner.tasks)
        {
            inner.tasks.remove(task.spec.id.as_str());
            return Err(StoreError::Persistence(error));
        }
        Ok(task)
    }

    pub async fn get(&self, id: &str, now: u64) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        let swept = sweep_expired(&mut inner.tasks, now);
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
        let swept = sweep_expired(&mut inner.tasks, now);

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
            executor_attached: false,
        })
    }

    /// Annule une tâche non terminale. L'état n'est reconnu qu'après écriture
    /// durable : un échec de persistance laisse la tâche inchangée.
    pub async fn cancel(&self, id: &str, now: u64) -> Result<TaskRecord, StoreError> {
        let mut inner = self.inner.lock().await;
        sweep_expired(&mut inner.tasks, now);

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
        Ok(task)
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
