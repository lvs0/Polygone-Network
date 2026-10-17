//! net — real network transport over the blind relay (plane 2 of the spec).
//!
//! ```text
//! Alice (sender)                        Bob (receiver)
//!   │                                    │
//!   │  HELLO <node_id>                   │  HELLO <node_id>
//!   ├──────────────────────► relay ◄─────┤
//!   │  HELLO_OK / HELLO_DENIED           │  (registration acknowledged)
//!   │  fragment envelopes (7 + kem_ct)   │
//!   │───────────────────────►────────────┤  forward verbatim
//!   │                                    │  buffer by session → ≥4/7 → decrypt
//! ```
//!
//! Wire contract (NDJSON over TCP):
//! ```json
//! {"kind":"fragment","from":"..","to":"..","session":"..","seq":0,
//!  "type":"kem"|"frag","idx":0,"threshold":4,"total":7,"payload":[...]}
//! ```
//! The relay only reads kind/to/session. The payload is opaque bytes.

use anyhow::Result;
use polygone_core::crypto::{kem, shamir, symmetric};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::identity::LocalIdentity;
use crate::msg;

/// Keep in sync with the relay's 64 KiB NDJSON line cap.
pub(crate) const MAX_RELAY_LINE_BYTES: usize = 64 * 1024;
/// Bound the bytes retained for incomplete network file transfers.
const MAX_PENDING_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_PENDING_FILE_TRANSFERS: usize = 128;
const MAX_FILE_CHUNKS: usize = MAX_PENDING_FILE_BYTES.div_ceil(crate::drive::CHUNK_SIZE);

/// Trusted peers persisted at `~/.polygone/peers.json`: `from` (node_id)
/// → the ML-DSA public key we expect from that peer (hex). This is the
/// real trust anchor of « c'est bien Alice » — learned at first contact
/// (TOFU), then enforced: a known peer cannot be impersonated.
pub fn peers_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".polygone").join("peers.json")
}

const PEERS_MARKER: &[u8] = b"polygone-peers-store-v1\n";
const PEERS_TRANSACTION_MARKER: &[u8] = b"polygone-peers-transaction-v1\n";
static PEERS_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Load the trust anchor. A missing or invalid store is an error, never TOFU.
pub fn load_peers() -> Result<HashMap<String, String>> {
    load_peers_at(&peers_path())
}

/// Explicit first-run bootstrap. Existing stores are never overwritten.
pub fn initialize_peers() -> Result<()> {
    initialize_peers_at(&peers_path())
}

pub(crate) fn ensure_peers_store_recovery_clear_at(path: &Path) -> Result<()> {
    let _lock = lock_peer_store(path)?;
    ensure_no_unresolved_peer_transaction(path)
}

/// Persist the trust anchor using an atomic same-directory replacement.
/// On Unix, the implementation calls `sync_all` on the parent directory where
/// the platform/filesystem supports it. This is not a universal hardware- or
/// filesystem-level crash-durability guarantee. Elsewhere, the file is synced
/// before rename, but the standard library offers no portable directory
/// flush, so directory-entry crash durability is not claimed.
#[allow(dead_code)] // Kept as the fallible store API; protocol paths commit under their snapshot lock.
pub fn save_peers(peers: &HashMap<String, String>) -> Result<()> {
    let path = peers_path();
    let _lock = lock_peer_store(&path)?;
    save_peers_at_locked(&path, peers)
}

fn peer_marker_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".initialized");
    PathBuf::from(name)
}

fn peer_transaction_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".transaction");
    PathBuf::from(name)
}

fn peer_backup_prefix(path: &Path) -> Result<String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("chemin de store invalide : {}", path.display()))?
        .to_string_lossy();
    Ok(format!(".{file_name}.backup-"))
}

fn ensure_no_unresolved_target_transaction(target: &Path) -> Result<()> {
    let transaction = peer_transaction_path(target);
    match std::fs::symlink_metadata(&transaction) {
        Ok(_) => anyhow::bail!(
            "transaction du store de confiance non résolue : {}; récupération explicite requise avant tout chargement (inspectez les artefacts, restaurez une sauvegarde vérifiée ou confirmez l’état du store, puis retirez les résidus uniquement après validation)",
            transaction.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let backup_prefix = peer_backup_prefix(target)?;
    let entries = match std::fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(&backup_prefix)
        {
            anyhow::bail!(
                "sauvegarde du store de confiance non résolue : {}; récupération explicite requise avant tout chargement (inspectez le store et ses sauvegardes, puis retirez les marqueurs uniquement après validation)",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn ensure_no_unresolved_peer_transaction(path: &Path) -> Result<()> {
    ensure_no_unresolved_target_transaction(path)?;
    ensure_no_unresolved_target_transaction(&peer_marker_path(path))?;
    Ok(())
}

#[cfg(not(unix))]
fn peer_lock_path(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = parent
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "store".to_string());
    peer_store_lock_scope(path).join(format!(".{directory}.peers.lock"))
}

fn peer_store_lock_scope(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent
        .parent()
        .filter(|scope| !scope.as_os_str().is_empty())
        .unwrap_or(parent)
        .to_path_buf()
}

#[cfg(unix)]
fn lock_peer_store(path: &Path) -> Result<std::fs::File> {
    let scope = peer_store_lock_scope(path);
    std::fs::create_dir_all(&scope)?;
    // Lock an existing ancestor so initialization and purge serialize even
    // before the store's own directory has been created.
    let lock = std::fs::File::open(scope)?;
    lock.lock()?;
    Ok(lock)
}

pub(crate) fn remove_peers_store() -> Result<bool> {
    remove_peers_at(&peers_path())
}

fn remove_peers_at(path: &Path) -> Result<bool> {
    remove_peers_at_with_sync(path, sync_parent_directory)
}

fn remove_peers_at_with_sync<S>(path: &Path, mut sync_parent: S) -> Result<bool>
where
    S: FnMut(&Path) -> std::io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let _lock = lock_peer_store(path)?;
    match std::fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => anyhow::bail!(
            "répertoire du store de confiance invalide : {}",
            parent.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }

    let transaction = peer_transaction_path(path);
    let transaction_preexisted = match std::fs::symlink_metadata(&transaction) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_transaction_marker(path, parent, &mut sync_parent)?;
            false
        }
        Err(error) => return Err(error.into()),
    };
    let marker = peer_marker_path(path);
    let targets = [
        path.to_path_buf(),
        marker.clone(),
        peer_transaction_path(&marker),
    ];
    for target in &targets {
        match std::fs::symlink_metadata(target) {
            Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
            }
            Ok(_) => anyhow::bail!(
                "cible non régulière dans le store de confiance; purge interrompue : {}",
                target.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let bases = [path, marker.as_path()]
        .into_iter()
        .filter_map(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let mut residue_paths = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_persistence_residue = bases.iter().any(|base| {
            name.starts_with(&format!(".{base}.tmp-"))
                || name.starts_with(&format!(".{base}.backup-"))
        });
        if is_persistence_residue {
            let file_type = entry.file_type()?;
            if file_type.is_file() || file_type.is_symlink() {
                residue_paths.push(entry.path());
            } else {
                anyhow::bail!(
                    "résidu de persistance non régulier; purge interrompue et garde conservé : {}",
                    entry.path().display()
                );
            }
        }
    }

    let mut removed = false;
    for target in targets {
        match std::fs::symlink_metadata(&target) {
            Ok(_) => {
                std::fs::remove_file(target)?;
                removed = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    for residue in residue_paths {
        std::fs::remove_file(residue)?;
        removed = true;
    }

    // Keep the transaction guard until all deletions and residual cleanup are
    // durable. If either sync fails, subsequent trust loads remain blocked.
    sync_parent(parent)?;
    if let Err(clear_error) = clear_transaction_marker(&transaction, parent, &mut sync_parent) {
        match ensure_transaction_marker(path, parent, &mut sync_parent) {
            Ok(guard) => anyhow::bail!(
                "purge du store effectuée mais retrait du garde non synchronisé ({clear_error}); garde rétabli dans {}",
                guard.display()
            ),
            Err(guard_error) => anyhow::bail!(
                "purge du store effectuée mais retrait du garde non synchronisé ({clear_error}); rétablissement du garde échoué ({guard_error}); le store absent reste bloqué jusqu’à une initialisation explicite"
            ),
        };
    }
    removed |= transaction_preexisted;
    Ok(removed)
}

#[cfg(not(unix))]
fn lock_peer_store(path: &Path) -> Result<std::fs::File> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let scope = peer_store_lock_scope(path);
    std::fs::create_dir_all(&scope)?;
    // On platforms where directory handles cannot be locked, use a dedicated
    // sidecar and the same std::fs::File::lock primitive.
    let lock_path = peer_lock_path(path);
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    let lock = options.open(&lock_path)?;
    lock.lock()?;
    enforce_private_mode(&lock_path)?;
    Ok(lock)
}

fn marker_exists(path: &Path) -> Result<bool> {
    match std::fs::read(path) {
        Ok(contents) if contents == PEERS_MARKER => Ok(true),
        Ok(_) => anyhow::bail!(
            "marqueur du store de confiance invalide : {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn initialize_peers_at(path: &Path) -> Result<()> {
    let _lock = lock_peer_store(path)?;
    initialize_peers_at_locked(path)
}

fn initialize_peers_at_locked(path: &Path) -> Result<()> {
    ensure_no_unresolved_peer_transaction(path)?;
    let marker = peer_marker_path(path);
    if marker_exists(&marker)? {
        anyhow::bail!(
            "le store de confiance est déjà initialisé : {}",
            path.display()
        );
    }
    match std::fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!(
            "un store existe déjà sans marqueur; chargez-le pour le migrer au lieu de le réinitialiser : {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let empty = HashMap::<String, String>::new();
    let raw = serde_json::to_vec_pretty(&empty)?;
    atomic_write_bytes(path, &raw)?;
    atomic_write_bytes(&marker, PEERS_MARKER)?;
    Ok(())
}

fn load_peers_at(path: &Path) -> Result<HashMap<String, String>> {
    load_peers_at_with(path, |store_path| std::fs::read_to_string(store_path))
}

fn load_peers_at_with<F>(path: &Path, read_store: F) -> Result<HashMap<String, String>>
where
    F: FnOnce(&Path) -> std::io::Result<String>,
{
    let _lock = lock_peer_store(path)?;
    load_peers_at_with_locked(path, read_store)
}

fn load_peers_at_with_locked<F>(path: &Path, read_store: F) -> Result<HashMap<String, String>>
where
    F: FnOnce(&Path) -> std::io::Result<String>,
{
    ensure_no_unresolved_peer_transaction(path)?;
    let marker = peer_marker_path(path);
    let initialized = marker_exists(&marker)?;
    let raw = match read_store(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if initialized {
                anyhow::bail!(
                    "store de confiance disparu après initialisation : {}",
                    path.display()
                );
            }
            anyhow::bail!(
                "store de confiance absent; initialisez-le explicitement avec `polygone initialiser-pairs` : {}",
                path.display()
            );
        }
        Err(error) => return Err(error.into()),
    };
    let peers = serde_json::from_str::<HashMap<String, String>>(&raw).map_err(|error| {
        anyhow::anyhow!(
            "store de confiance JSON invalide ({}): {error}",
            path.display()
        )
    })?;

    // A valid pre-marker store is a legacy installation. Mark it only after
    // successfully reading and decoding it, so later disappearance is fatal.
    if !initialized {
        atomic_write_bytes(&marker, PEERS_MARKER)?;
    }
    enforce_private_mode(path)?;
    Ok(peers)
}

#[cfg(test)]
fn save_peers_at(path: &Path, peers: &HashMap<String, String>) -> Result<()> {
    let _lock = lock_peer_store(path)?;
    save_peers_at_locked(path, peers)
}

fn save_peers_at_locked(path: &Path, peers: &HashMap<String, String>) -> Result<()> {
    ensure_no_unresolved_peer_transaction(path)?;
    let marker = peer_marker_path(path);
    if !marker_exists(&marker)? {
        anyhow::bail!("store de confiance non initialisé : {}", path.display());
    }
    // A running process must not recreate a store that disappeared after
    // startup, nor overwrite a store that became unreadable or corrupt.
    let current = std::fs::read_to_string(path).map_err(|error| {
        anyhow::anyhow!(
            "lecture du store de confiance impossible ({}): {error}",
            path.display()
        )
    })?;
    serde_json::from_str::<HashMap<String, String>>(&current).map_err(|error| {
        anyhow::anyhow!(
            "store de confiance JSON invalide ({}): {error}",
            path.display()
        )
    })?;
    enforce_private_mode(path)?;
    let raw = serde_json::to_vec_pretty(peers)?;
    atomic_write_bytes(path, &raw)
}

#[cfg(unix)]
fn enforce_private_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != 0o600 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn enforce_private_mode(_path: &Path) -> Result<()> {
    Ok(())
}

fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_bytes_with(path, bytes, |file, contents| file.write_all(contents))
}

fn atomic_write_bytes_with<F>(path: &Path, bytes: &[u8], write: F) -> Result<()>
where
    F: FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
{
    atomic_write_bytes_with_hooks(path, bytes, write, sync_parent_directory)
}

fn atomic_write_bytes_with_hooks<F, S>(
    path: &Path,
    bytes: &[u8],
    write: F,
    mut sync_parent: S,
) -> Result<()>
where
    F: FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    S: FnMut(&Path) -> std::io::Result<()>,
{
    atomic_write_bytes_with_full_hooks(
        path,
        bytes,
        write,
        |from, to| std::fs::rename(from, to),
        |backup| std::fs::remove_file(backup),
        |parent| sync_parent(parent),
    )
}

fn atomic_write_bytes_with_full_hooks<F, R, D, S>(
    path: &Path,
    bytes: &[u8],
    write: F,
    mut replace: R,
    mut remove_backup: D,
    mut sync_parent: S,
) -> Result<()>
where
    F: FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    D: FnMut(&Path) -> std::io::Result<()>,
    S: FnMut(&Path) -> std::io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    ensure_no_unresolved_target_transaction(path)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("chemin de store invalide : {}", path.display()))?
        .to_string_lossy();

    let (temp_path, mut temp_file) = loop {
        let counter = PEERS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{file_name}.tmp-{}-{counter}", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };

    let mut backup_path = None;
    let result = (|| -> Result<()> {
        enforce_private_mode(&temp_path)?;
        write(&mut temp_file, bytes)?;
        temp_file.sync_all()?;
        drop(temp_file);

        // This marker is synced before any replacement. If this process is
        // interrupted or any rollback/cleanup step is uncertain, subsequent
        // loads refuse the store until an operator explicitly recovers it.
        let transaction = create_transaction_marker(path, parent, &mut sync_parent)?;
        backup_path = backup_existing_file(path, parent, &file_name)?;
        if let Err(rename_error) = replace(&temp_path, path) {
            let backup_status = backup_path
                .as_ref()
                .map(|backup| format!("sauvegarde précédente conservée dans {}", backup.display()))
                .unwrap_or_else(|| "aucun store antérieur à restaurer".to_string());
            anyhow::bail!(
                "le remplacement atomique a renvoyé une erreur ({rename_error}) sans prouver qu’il n’a pas eu lieu; {backup_status}; marqueur conservé dans {}; état indéterminé et récupération explicite requise avant tout chargement",
                transaction.display()
            );
        }
        if let Err(sync_error) = sync_parent(parent) {
            let rollback = match backup_path.as_ref() {
                Some(backup) => {
                    restore_backup_from_copy(backup, path, parent, &file_name, &mut replace)
                }
                None => std::fs::remove_file(path).map_err(Into::into),
            };
            if let Err(rollback_error) = rollback {
                let detail = backup_path
                    .as_ref()
                    .map(|backup| format!("; ancienne copie conservée dans {}", backup.display()))
                    .unwrap_or_default();
                anyhow::bail!(
                    "synchronisation du dossier après remplacement échouée ({sync_error}); restauration échouée ({rollback_error}){detail}; transaction conservée dans {} et récupération explicite requise",
                    transaction.display()
                );
            }
            if let Err(restore_sync_error) = sync_parent(parent) {
                anyhow::bail!(
                    "remplacement refusé ({sync_error}); restauration visible mais synchronisation du rollback échouée ({restore_sync_error}); transaction conservée dans {} et récupération explicite requise",
                    transaction.display()
                );
            }
            if let Some(backup) = backup_path.as_ref() {
                remove_backup_confirmed(backup, parent, &mut remove_backup, &mut sync_parent)?;
            }
            backup_path = None;
            clear_transaction_marker(&transaction, parent, &mut sync_parent)?;
            return Err(sync_error.into());
        }
        if let Err(commit_error) = clear_transaction_marker(&transaction, parent, &mut sync_parent)
        {
            let rollback_transaction = ensure_transaction_marker(path, parent, &mut sync_parent)?;
            let rollback = match backup_path.as_ref() {
                Some(backup) => {
                    restore_backup_from_copy(backup, path, parent, &file_name, &mut replace)
                }
                None => match std::fs::remove_file(path) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    result => result.map_err(Into::into),
                },
            };
            if let Err(rollback_error) = rollback {
                let detail = backup_path
                    .as_ref()
                    .map(|backup| format!("; ancienne copie conservée dans {}", backup.display()))
                    .unwrap_or_default();
                anyhow::bail!(
                    "synchronisation de la validation du store échouée ({commit_error}); restauration échouée ({rollback_error}){detail}; transaction conservée dans {} et récupération explicite requise",
                    rollback_transaction.display()
                );
            }
            if let Err(restore_sync_error) = sync_parent(parent) {
                anyhow::bail!(
                    "validation du store échouée ({commit_error}); restauration visible mais synchronisation du rollback échouée ({restore_sync_error}); transaction conservée dans {} et récupération explicite requise",
                    rollback_transaction.display()
                );
            }
            if let Some(backup) = backup_path.as_ref() {
                remove_backup_confirmed(backup, parent, &mut remove_backup, &mut sync_parent)?;
            }
            backup_path = None;
            clear_transaction_marker(&rollback_transaction, parent, &mut sync_parent)?;
            return Err(commit_error);
        }
        if let Some(backup) = backup_path.as_ref() {
            if let Err(cleanup_error) =
                remove_backup_confirmed(backup, parent, &mut remove_backup, &mut sync_parent)
            {
                // The new store is already committed. If the backup is still
                // present (or its cleanup durability is uncertain), retain a
                // transaction marker instead of attempting a rollback from a
                // backup that may have been removed despite an error return.
                let transaction = create_transaction_marker(path, parent, &mut sync_parent)?;
                anyhow::bail!(
                    "nettoyage de la copie de secours échoué ({cleanup_error}); store transactionnel conservé; récupération explicite requise dans {}",
                    transaction.display()
                );
            }
            backup_path = None;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

fn restore_backup_from_copy<R>(
    backup: &Path,
    path: &Path,
    parent: &Path,
    file_name: &str,
    replace: &mut R,
) -> Result<()>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    let (restore_path, mut restore_file) = loop {
        let counter = PEERS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.tmp-restore-{}-{counter}",
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    };

    let copy_result = (|| -> std::io::Result<()> {
        let mut backup_file = std::fs::File::open(backup)?;
        std::io::copy(&mut backup_file, &mut restore_file)?;
        restore_file.sync_all()
    })();
    if let Err(error) = copy_result {
        drop(restore_file);
        let _ = std::fs::remove_file(&restore_path);
        return Err(error.into());
    }
    drop(restore_file);

    let result = replace(&restore_path, path);
    if result.is_err() {
        let _ = std::fs::remove_file(&restore_path);
    }
    result.map_err(Into::into)
}

fn remove_backup_confirmed<D, S>(
    backup: &Path,
    parent: &Path,
    remove_backup: &mut D,
    sync_parent: &mut S,
) -> Result<()>
where
    D: FnMut(&Path) -> std::io::Result<()>,
    S: FnMut(&Path) -> std::io::Result<()>,
{
    let remove_error = remove_backup(backup).err();
    match std::fs::symlink_metadata(backup) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A filesystem/hook may report an error after unlinking. The
            // deletion is complete only after the containing directory sync.
        }
        Ok(_) => match remove_error {
            Some(error) => anyhow::bail!(
                "suppression de la sauvegarde échouée ({error}); copie toujours présente dans {}",
                backup.display()
            ),
            None => anyhow::bail!(
                "suppression de la sauvegarde annoncée réussie mais copie toujours présente dans {}",
                backup.display()
            ),
        },
        Err(error) => anyhow::bail!(
            "impossible de vérifier la sauvegarde après sa suppression dans {} ({error})",
            backup.display()
        ),
    }
    sync_parent(parent).map_err(|error| {
        anyhow::anyhow!(
            "synchronisation de la suppression de la sauvegarde échouée dans {} ({error})",
            parent.display()
        )
    })
}

fn create_transaction_marker<S>(path: &Path, parent: &Path, sync_parent: &mut S) -> Result<PathBuf>
where
    S: FnMut(&Path) -> std::io::Result<()>,
{
    let transaction = peer_transaction_path(path);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&transaction)?;
    file.write_all(PEERS_TRANSACTION_MARKER)?;
    file.sync_all()?;
    drop(file);
    sync_parent(parent)?;
    Ok(transaction)
}

fn ensure_transaction_marker<S>(path: &Path, parent: &Path, sync_parent: &mut S) -> Result<PathBuf>
where
    S: FnMut(&Path) -> std::io::Result<()>,
{
    let transaction = peer_transaction_path(path);
    match std::fs::symlink_metadata(&transaction) {
        Ok(_) => Ok(transaction),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_transaction_marker(path, parent, sync_parent)
        }
        Err(error) => Err(error.into()),
    }
}

fn clear_transaction_marker<S>(path: &Path, parent: &Path, sync_parent: &mut S) -> Result<()>
where
    S: FnMut(&Path) -> std::io::Result<()>,
{
    std::fs::remove_file(path).map_err(|error| {
        anyhow::anyhow!(
            "transaction du store de confiance non résolue après écriture : {}; récupération explicite requise ({error})",
            path.display()
        )
    })?;
    sync_parent(parent).map_err(|error| {
        anyhow::anyhow!(
            "suppression du marqueur de transaction non synchronisée : {}; récupération explicite requise ({error})",
            path.display()
        )
    })
}

fn backup_existing_file(path: &Path, parent: &Path, file_name: &str) -> Result<Option<PathBuf>> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }

    loop {
        let counter = PEERS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let backup = parent.join(format!(
            ".{file_name}.backup-{}-{counter}",
            std::process::id()
        ));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut backup_file = match options.open(&backup) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let copy_result = (|| -> std::io::Result<()> {
            let mut old_file = std::fs::File::open(path)?;
            std::io::copy(&mut old_file, &mut backup_file)?;
            backup_file.sync_all()
        })();
        if let Err(error) = copy_result {
            let _ = std::fs::remove_file(&backup);
            return Err(error.into());
        }
        return Ok(Some(backup));
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> std::io::Result<()> {
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> std::io::Result<()> {
    // The standard library does not expose a portable directory-handle flush.
    Ok(())
}

pub(crate) fn persist_peer_changes_at(
    known_peers: &mut HashMap<String, String>,
    previous: &HashMap<String, String>,
    store_path: &Path,
) -> Result<bool> {
    let persist = (|| -> Result<bool> {
        let _lock = lock_peer_store(store_path)?;
        let stored = load_peers_at_with_locked(store_path, |path| std::fs::read_to_string(path))?;
        let merged = merge_peer_snapshot(known_peers, previous, &stored, store_path)?;
        let changed = merged != stored;
        if changed {
            save_peers_at_locked(store_path, &merged)?;
        }
        *known_peers = merged;
        Ok(changed)
    })();
    match persist {
        Ok(changed) => Ok(changed),
        Err(error) => {
            *known_peers = previous.clone();
            Err(error)
        }
    }
}

fn merge_peer_snapshot(
    known_peers: &HashMap<String, String>,
    previous: &HashMap<String, String>,
    stored: &HashMap<String, String>,
    store_path: &Path,
) -> Result<HashMap<String, String>> {
    for (node, key) in previous {
        if stored.get(node) != Some(key) {
            anyhow::bail!(
                "une ancre connue a été supprimée ou modifiée dans le store de confiance : {}",
                store_path.display()
            );
        }
        if known_peers.get(node) != Some(key) {
            anyhow::bail!("refus de modifier ou supprimer une ancre déjà connue ({node})");
        }
    }

    let mut merged = stored.clone();
    for (node, key) in known_peers {
        if previous.contains_key(node) {
            continue;
        }
        match merged.get(node) {
            Some(existing) if existing != key => {
                anyhow::bail!(
                    "conflit de clé pour le pair {node}; refus de remplacer l’ancre persistée"
                );
            }
            Some(_) => {}
            None => {
                merged.insert(node.clone(), key.clone());
            }
        }
    }
    Ok(merged)
}

fn process_line_with_persistence(
    line: &str,
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    now_secs: u64,
    completed: &mut VecDeque<(String, u64)>,
    sessions: &mut HashMap<String, SessionBuffer>,
    store_path: &Path,
) -> Result<Option<(String, Received)>> {
    let previous = known_peers.clone();
    let received = process_line(line, identity, known_peers, now_secs, completed, sessions)?;
    if received.is_some() {
        persist_peer_changes_at(known_peers, &previous, store_path)?;
    }
    Ok(received)
}

/// A network envelope, as JSON on the wire.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct NetEnvelope {
    pub(crate) kind: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) session: String,
    /// Vestigial : numéro de fragment informatif, JAMAIS validé à la
    /// réception. L'anti-replay réel est l'horodatage signé ±300 s +
    /// le cache des sessions complétées (README, testé) — plus fort
    /// qu'un seq strict, qui resterait rejouable dans la fenêtre.
    pub(crate) seq: u64,
    #[serde(rename = "type")]
    pub(crate) typ: String,
    pub(crate) idx: u8,
    pub(crate) threshold: u8,
    pub(crate) total: u8,
    pub(crate) payload: Vec<u8>,
    /// Present only on the KEM envelope: ML-DSA-65 signature over the
    /// canonical message. "C'est bien Alice" — verified by the receiver
    /// before decrypt.
    #[serde(default)]
    pub(crate) sig: Option<Vec<u8>>,
    /// Present only on the KEM envelope: sender's ML-DSA-65 public key (hex).
    #[serde(default)]
    pub(crate) signer: Option<String>,
    /// Present only on the KEM envelope of a file transfer: the file name
    /// encrypted with the session key (`symmetric::encrypt` output). The
    /// relay sees only opaque bytes — the name is out-of-band.
    #[serde(default)]
    pub(crate) name_ct: Option<Vec<u8>>,
    /// Present only on the KEM envelope: sender clock, unix seconds. Bound
    /// into the signature (freshness) — a captured session cannot be
    /// replayed outside the ± window.
    #[serde(default)]
    pub(crate) ts: Option<u64>,
}

/// The node id of an identity: first 16 hex chars of the KEM public key.
pub fn node_id(identity: &LocalIdentity) -> String {
    identity.kem_pk_hex.chars().take(16).collect()
}

/// Max accepted skew between the sender's timestamp and our clock (anti-replay).
pub const MAX_TS_SKEW_SECS: u64 = 300;
/// Max buffered sessions (anti-DoS memory) — fail closed beyond the cap.
pub const MAX_SESSIONS: usize = 1024;
/// Incomplete sessions are purged after this long without activity.
pub const SESSION_TTL_SECS: u64 = 300;
/// Cap on the completed-session anti-replay cache.
pub const MAX_COMPLETED: usize = 4096;
/// A timestamp may be 300 seconds ahead, then remain valid for 300 more.
const COMPLETED_RETENTION_SECS: u64 = 2 * MAX_TS_SKEW_SECS;

/// Canonical bytes signed by the sender and verified by the receiver.
///
/// Deterministic: the receiver rebuilds the exact same bytes from the
/// session metadata + the reconstructed ciphertext (Shamir rebuilds the
/// same secret from any 4-of-7 fragments), so verification needs no
/// additional fields on the wire. `ts` binds freshness into the signature:
/// a replayed session fails the clock window.
pub fn canonical_bytes(
    session: &str,
    from: &str,
    to: &str,
    ts: u64,
    kem_ct: &[u8],
    ciphertext: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        64 + session.len() + from.len() + to.len() + kem_ct.len() + ciphertext.len(),
    );
    out.extend_from_slice(b"polygone-net-v2\0");
    out.extend_from_slice(session.as_bytes());
    out.push(0);
    out.extend_from_slice(from.as_bytes());
    out.push(0);
    out.extend_from_slice(to.as_bytes());
    out.push(0);
    out.extend_from_slice(&ts.to_be_bytes());
    out.push(0);
    out.extend_from_slice(kem_ct);
    out.push(0);
    out.extend_from_slice(ciphertext);
    out
}

/// Random session id (hex), from the OS CSPRNG.
pub(crate) fn session_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

/// Connect to the relay and register our node id.
///
/// The relay acknowledges the registration with `HELLO_OK` (the slot is
/// ours) or `HELLO_DENIED` (another connection already owns this node_id).
/// We WAIT for the ack: a denied registration must never silently send its
/// fragments into a slot it does not own. The ack is read byte-by-byte so
/// nothing beyond the line is buffered — the same connection carries the
/// actual traffic right after.
async fn connect(relay: &str, identity: &LocalIdentity) -> Result<TcpStream> {
    let mut stream = TcpStream::connect(relay).await?;
    let id = node_id(identity);
    stream.write_all(format!("HELLO {id}\n").as_bytes()).await?;
    let mut ack = Vec::with_capacity(16);
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).await?;
        if byte[0] == b'\n' {
            break;
        }
        ack.push(byte[0]);
        if ack.len() > 32 {
            anyhow::bail!("relay: réponse HELLO illisible");
        }
    }
    let ack = String::from_utf8_lossy(&ack);
    if ack.trim() != "HELLO_OK" {
        anyhow::bail!(
            "relay a refusé l'enregistrement de {id}: {ack} — un autre nœud utilise déjà cet identifiant ?"
        );
    }
    Ok(stream)
}

pub(crate) async fn connect_relay(relay: &str, identity: &LocalIdentity) -> Result<TcpStream> {
    connect(relay, identity).await
}

/// Serialize one wire envelope and reject it before connecting if the relay
/// would drop it for exceeding its NDJSON line cap.
fn serialize_wire_line(env: &NetEnvelope) -> Result<String> {
    let line = serde_json::to_string(env)?;
    if line.len() > MAX_RELAY_LINE_BYTES {
        anyhow::bail!(
            "enveloppe trop volumineuse pour le relay ({} octets, limite {}); réduisez la taille du fichier/bloc",
            line.len(),
            MAX_RELAY_LINE_BYTES
        );
    }
    Ok(line)
}

/// Read one NDJSON line without allowing an untrusted relay to grow the buffer
/// beyond its line cap. The caller keeps `line` across select! cancellations.
pub(crate) async fn read_line_capped<R>(
    reader: &mut R,
    line: &mut Vec<u8>,
    cap: usize,
) -> std::io::Result<usize>
where
    R: AsyncBufRead + Unpin,
{
    let max_with_newline = cap.checked_add(1).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "limite de ligne invalide")
    })?;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // A trailing partial line is not a valid NDJSON record.
            return Ok(0);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        let remaining = max_with_newline.saturating_sub(line.len());
        if take > remaining {
            if remaining > 0 {
                line.extend_from_slice(&available[..remaining]);
                reader.consume(remaining);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ligne du relais supérieure à la limite de {cap} octets"),
            ));
        }

        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            return Ok(line.len());
        }
        if line.len() > cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ligne du relais supérieure à la limite de {cap} octets"),
            ));
        }
    }
}

/// Send a message (or a file, when `name` is set) to a peer through the
/// relay. Returns the session id.
pub async fn send_network(
    relay: &str,
    dest_node: &str,
    payload: &[u8],
    name: Option<&str>,
    recipient_pk: &kem::KemPublicKey,
    identity: &LocalIdentity,
) -> Result<String> {
    let output = msg::send_bytes(payload, recipient_pk)?;
    let mut sessions = send_network_outputs(
        relay,
        dest_node,
        std::slice::from_ref(&output),
        name,
        identity,
    )
    .await?;
    Ok(sessions.remove(0))
}

/// Encrypt a file into Drive chunks and send each chunk as an independently
/// signed network session. All lines are size-checked before connecting.
pub async fn send_network_file(
    relay: &str,
    dest_node: &str,
    payload: &[u8],
    name: &str,
    recipient_pk: &kem::KemPublicKey,
    identity: &LocalIdentity,
) -> Result<Vec<String>> {
    if payload.len() > MAX_PENDING_FILE_BYTES {
        anyhow::bail!(
            "fichier trop volumineux pour le transfert réseau (limite {} Mio)",
            MAX_PENDING_FILE_BYTES / (1024 * 1024)
        );
    }
    let total_chunks = payload.len().max(1).div_ceil(crate::drive::CHUNK_SIZE);
    let total_wire = u32::try_from(total_chunks)
        .map_err(|_| anyhow::anyhow!("fichier trop volumineux pour le format Drive"))?;
    let transfer_id = crate::drive::DriveFile::new_transfer_id();
    let first_end = payload.len().min(crate::drive::CHUNK_SIZE);
    let first_output = crate::drive::DriveFile::prepare_send_chunk(
        &payload[..first_end],
        name,
        recipient_pk,
        transfer_id,
        0,
        total_wire,
    )?;
    let first_session = session_id();
    // The first chunk is the largest one. Preflight its complete wire group
    // before connecting; all following chunks are no larger and use the same
    // metadata lengths.
    let first_lines = network_lines_for_output(
        &first_session,
        dest_node,
        &first_output,
        Some(name),
        identity,
    )?;
    let mut stream = connect(relay, identity).await?;
    for line in first_lines {
        stream.write_all(format!("{line}\n").as_bytes()).await?;
    }

    let mut sessions = Vec::with_capacity(total_chunks);
    sessions.push(first_session);
    for seq in 1..total_chunks {
        // One encrypted Drive block is 8 relay lines (KEM + 7 shares). Send
        // at most 24 blocks per rate-limit window, then let its 200-line
        // token bucket refill before continuing.
        if seq > 0 && seq.is_multiple_of(24) {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        let start = seq * crate::drive::CHUNK_SIZE;
        let end = start
            .saturating_add(crate::drive::CHUNK_SIZE)
            .min(payload.len());
        let output = crate::drive::DriveFile::prepare_send_chunk(
            &payload[start..end],
            name,
            recipient_pk,
            transfer_id,
            seq,
            total_wire,
        )?;
        let session = session_id();
        let lines = network_lines_for_output(&session, dest_node, &output, Some(name), identity)?;
        for line in lines {
            stream.write_all(format!("{line}\n").as_bytes()).await?;
        }
        sessions.push(session);
    }
    stream.flush().await?;
    Ok(sessions)
}

async fn send_network_outputs(
    relay: &str,
    dest_node: &str,
    outputs: &[msg::SendOutput],
    name: Option<&str>,
    identity: &LocalIdentity,
) -> Result<Vec<String>> {
    if outputs.is_empty() {
        anyhow::bail!("aucun bloc à envoyer");
    }
    let sessions: Vec<String> = outputs.iter().map(|_| session_id()).collect();

    // Validate every envelope before opening the relay connection, without
    // retaining wire copies proportional to the entire file size.
    for (output, session) in outputs.iter().zip(&sessions) {
        let _ = network_lines_for_output(session, dest_node, output, name, identity)?;
    }

    let mut stream = connect(relay, identity).await?;
    for (output, session) in outputs.iter().zip(&sessions) {
        let lines = network_lines_for_output(session, dest_node, output, name, identity)?;
        for line in lines {
            stream.write_all(format!("{line}\n").as_bytes()).await?;
        }
    }
    stream.flush().await?;
    Ok(sessions)
}

fn network_lines_for_output(
    session: &str,
    dest_node: &str,
    output: &msg::SendOutput,
    name: Option<&str>,
    identity: &LocalIdentity,
) -> Result<Vec<String>> {
    let from = node_id(identity);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ciphertext = output
        .ciphertext
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("chiffrement manquant pour l’envoi réseau"))?;

    // ML-DSA-65 signature over the canonical message — "c'est bien Alice".
    let signer_pk = identity.sign_verifier()?;
    let sig = identity.sign_signer()?.sign(&canonical_bytes(
        session,
        &from,
        dest_node,
        ts,
        output.kem_ct.as_bytes(),
        ciphertext,
    ));

    // The file name remains opaque to the relay and readable only by the
    // recipient who derives the session key.
    let name_ct = match (name, &output.session_key) {
        (Some(n), Some(key)) => Some(symmetric::encrypt(n.as_bytes(), key)?),
        (Some(_), None) => anyhow::bail!("clé de session absente pour chiffrer le nom"),
        (None, _) => None,
    };

    let kem_env = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: dest_node.to_string(),
        session: session.to_string(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        sig: Some(sig.as_bytes().to_vec()),
        signer: Some(signer_pk.public_key().to_hex()),
        name_ct,
        ts: Some(ts),
    };
    let mut lines = Vec::with_capacity(output.fragments.len() + 1);
    lines.push(serialize_wire_line(&kem_env)?);

    for (i, frag) in output.fragments.iter().enumerate() {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: from.clone(),
            to: dest_node.to_string(),
            session: session.to_string(),
            seq: i as u64 + 1,
            typ: "frag".into(),
            idx: frag.index,
            threshold: 4,
            total: 7,
            payload: frag.share.clone(),
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };
        lines.push(serialize_wire_line(&env)?);
    }
    Ok(lines)
}

/// Borrow compute from a ghost node (RES lending, MVP).
///
/// Protocol: connect to the relay, register, send a `req` envelope to the
/// ghost node, then listen on the same connection for its `grant` reply
/// (the ghost must be listening with `ecouter --compute`).
pub async fn borrow_compute(
    relay: &str,
    ghost_node: &str,
    task: &str,
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let body = serde_json::json!({"action": "compute", "task": task});
    borrow_request(relay, ghost_node, body, identity, known_peers, timeout).await
}

/// Send a WASM module to a ghost node for sandboxed execution.
pub async fn borrow_wasm(
    relay: &str,
    ghost_node: &str,
    wasm: &[u8],
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let body = serde_json::json!({
        "action": "wasm",
        "wasm": base64_encode(wasm),
    });
    borrow_request(relay, ghost_node, body, identity, known_peers, timeout).await
}

/// Minimal base64 encoder (no external dep).
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The shared request/grant exchange (borrow or wasm).
/// `known_peers` is the trust anchor for the ghost's signing key (TOFU).
async fn borrow_request(
    relay: &str,
    ghost_node: &str,
    body: serde_json::Value,
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    borrow_request_with_store_path(
        relay,
        ghost_node,
        body,
        identity,
        known_peers,
        timeout,
        &peers_path(),
    )
    .await
}

async fn borrow_request_with_store_path(
    relay: &str,
    ghost_node: &str,
    body: serde_json::Value,
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    timeout: std::time::Duration,
    store_path: &Path,
) -> Result<Option<String>> {
    let mut stream = connect(relay, identity).await?;
    let from = node_id(identity);
    let req_session = session_id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // The request is signed (like messages): the ghost can verify who asks
    // for compute, and the grant is bound to OUR session.
    let body_bytes = body.to_string().into_bytes();
    let req_sig = identity.sign_signer()?.sign(&canonical_bytes(
        &req_session,
        &from,
        ghost_node,
        ts,
        &[],
        &body_bytes,
    ));

    let req = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: ghost_node.to_string(),
        session: req_session.clone(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: body_bytes,
        sig: Some(req_sig.as_bytes().to_vec()),
        signer: Some(identity.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(ts),
    };
    stream
        .write_all(format!("{}\n", serde_json::to_string(&req)?).as_bytes())
        .await?;
    stream.flush().await?;

    // Listen for the grant reply on the same connection. A grant is only
    // accepted if it answers OUR request: addressed to us, same session,
    // from the ghost we asked — anything else is a forged/poisoned grant.
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        line.clear();
        let n = tokio::time::timeout(
            timeout,
            read_line_capped(&mut reader, &mut line, MAX_RELAY_LINE_BYTES),
        )
        .await
        .unwrap_or(Ok(0))?;
        if n == 0 {
            break;
        }
        let env: NetEnvelope = match serde_json::from_slice(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if env.typ == "grant" {
            let ok =
                env.to == node_id(identity) && env.session == req_session && env.from == ghost_node;
            if ok {
                // The grant is signed by the ghost. If we already know the
                // ghost's key (trust anchor), the signature MUST verify —
                // a malicious relay cannot forge a grant for our session.
                // First contact = TOFU: learn the ghost's key.
                let previous = known_peers.clone();
                let sig_ok = verify_grant(&env, known_peers, ghost_node);
                if sig_ok {
                    persist_peer_changes_at(known_peers, &previous, store_path)?;
                    return Ok(Some(String::from_utf8_lossy(&env.payload).to_string()));
                }
            }
            // A mismatched or unverifiable grant is ignored, never trusted.
        }
    }
    Ok(None)
}

/// Verify a grant's signature against the ghost's known key (TOFU learn
/// on first contact).
fn verify_grant(
    env: &NetEnvelope,
    known_peers: &mut HashMap<String, String>,
    ghost_node: &str,
) -> bool {
    let (sig, signer, ts) = match (&env.sig, &env.signer, env.ts) {
        (Some(s), Some(pk_hex), Some(t)) if t > 0 => (s.clone(), pk_hex.clone(), t),
        _ => return false,
    };
    if let Some(expected_pk) = known_peers.get(ghost_node) {
        if expected_pk != &signer {
            return false; // claims to be a known ghost, signs as another
        }
    }
    let verifier = match polygone_core::sign::PublicKey::from_hex(&signer) {
        Ok(pk) => polygone_core::sign::Verifier::from_public(pk),
        Err(_) => return false,
    };
    let canonical = canonical_bytes(&env.session, &env.from, &env.to, ts, &[], &env.payload);
    let sig = polygone_core::sign::Signature::from_bytes(&sig);
    if !verifier.verify(&canonical, &sig) {
        return false;
    }
    // TOFU: learn the ghost's key on the first verified grant.
    if !known_peers.contains_key(ghost_node) {
        known_peers.insert(ghost_node.to_string(), signer);
    }
    true
}

/// A receiver-side session buffer.
#[derive(Default)]
struct SessionBuffer {
    from: String,
    to: String,
    kem_ct: Option<kem::KemCiphertext>,
    /// ML-DSA-65 signature + sender public key (from the KEM envelope).
    sig: Option<Vec<u8>>,
    signer: Option<String>,
    /// Encrypted file name (opaque bytes to the relay).
    name_ct: Option<Vec<u8>>,
    /// Sender clock (unix secs), bound into the signature.
    ts: Option<u64>,
    /// Last activity (unix secs) — stale sessions are purged.
    touched: u64,
    /// Seen fragment indices — duplicates and out-of-range are dropped.
    seen_idx: Vec<u8>,
    fragments: Vec<shamir::Fragment>,
}

/// What a completed network session produced.
pub(crate) enum Received {
    Message(String),
    File {
        name: String,
        bytes: Vec<u8>,
    },
    DriveChunk {
        sender: String,
        chunk: crate::drive::DecodedChunk,
    },
}

#[derive(Default)]
struct PendingDriveFile {
    name: String,
    total: usize,
    chunks: BTreeMap<usize, Vec<u8>>,
    bytes: usize,
    touched: u64,
}

#[derive(Default)]
struct PendingDriveFiles {
    files: HashMap<(String, [u8; 16]), PendingDriveFile>,
    buffered_bytes: usize,
}

impl PendingDriveFiles {
    fn discard(&mut self, key: &(String, [u8; 16])) {
        if let Some(file) = self.files.remove(key) {
            self.buffered_bytes = self.buffered_bytes.saturating_sub(file.bytes);
        }
    }

    fn expire_stale(&mut self, now_secs: u64) {
        let stale: Vec<_> = self
            .files
            .iter()
            .filter(|(_, file)| now_secs.saturating_sub(file.touched) >= SESSION_TTL_SECS)
            .map(|(key, _)| key.clone())
            .collect();
        for key in stale {
            self.discard(&key);
        }
    }

    fn accept(
        &mut self,
        sender: String,
        chunk: crate::drive::DecodedChunk,
        now_secs: u64,
    ) -> Result<Option<(String, Vec<u8>)>> {
        self.expire_stale(now_secs);

        if chunk.total == 0 || chunk.total > MAX_FILE_CHUNKS || chunk.seq >= chunk.total {
            anyhow::bail!("transfert Drive réseau hors limites");
        }
        let key = (sender, chunk.transfer_id);
        if !self.files.contains_key(&key) && self.files.len() >= MAX_PENDING_FILE_TRANSFERS {
            anyhow::bail!("trop de transferts Drive réseau incomplets");
        }
        if let Some(file) = self.files.get(&key) {
            if file.total != chunk.total || file.name != chunk.filename {
                self.discard(&key);
                anyhow::bail!("en-têtes incohérents dans le transfert Drive réseau");
            }
            if file.chunks.contains_key(&chunk.seq) {
                self.discard(&key);
                anyhow::bail!("bloc Drive réseau dupliqué");
            }
        }
        if self
            .buffered_bytes
            .checked_add(chunk.data.len())
            .is_none_or(|size| size > MAX_PENDING_FILE_BYTES)
        {
            self.discard(&key);
            anyhow::bail!("plafond mémoire des transferts Drive réseau atteint");
        }

        let file = self
            .files
            .entry(key.clone())
            .or_insert_with(|| PendingDriveFile {
                name: chunk.filename.clone(),
                total: chunk.total,
                ..PendingDriveFile::default()
            });
        file.touched = now_secs;
        file.bytes += chunk.data.len();
        file.chunks.insert(chunk.seq, chunk.data);
        self.buffered_bytes += file.chunks.get(&chunk.seq).map_or(0, Vec::len);

        if file.chunks.len() != file.total {
            return Ok(None);
        }

        let file = self
            .files
            .remove(&key)
            .expect("completed transfer is buffered");
        self.buffered_bytes = self.buffered_bytes.saturating_sub(file.bytes);
        let PendingDriveFile {
            name,
            total,
            chunks,
            ..
        } = file;
        let mut ordered = chunks.into_iter();
        let mut bytes = Vec::new();
        for expected_seq in 0..total {
            let (seq, mut chunk) = ordered
                .next()
                .ok_or_else(|| anyhow::anyhow!("bloc Drive réseau manquant"))?;
            if seq != expected_seq {
                anyhow::bail!("séquence de bloc Drive réseau manquante");
            }
            bytes.try_reserve(chunk.len()).map_err(|e| {
                anyhow::anyhow!("allocation du fichier réassemblé impossible : {e}")
            })?;
            bytes.append(&mut chunk);
        }
        Ok(Some((name, bytes)))
    }
}

/// Show a freshly learned peer (TOFU) so the user can verify the
/// fingerprint out-of-band before trusting it.
fn announce_new_peer(known_peers: &HashMap<String, String>, before: usize) {
    if known_peers.len() <= before {
        return;
    }
    for (node, pk_hex) in known_peers {
        // Only the new ones: trust is learned, not re-announced.
        if known_peers.len() == before + 1 {
            let fp: String = pk_hex.chars().take(16).collect();
            println!(
                "  ⬡ NOUVEAU PAIR APPRIS : {node} → {fp}… — vérifiez cette empreinte avec l'expéditeur (hors-ligne) avant de lui faire confiance."
            );
            return;
        }
    }
}

/// Build the base RES grant payload before it is signed.
fn grant_payload(identity: &LocalIdentity) -> serde_json::Value {
    serde_json::json!({
        "node": node_id(identity),
        "ram_mb": crate::mesh::free_ram_mb().unwrap_or(0),
        "ok": true,
    })
}

/// Build the RES grant reply for a compute request (ghost node). Pure —
/// testable without sockets. The final payload is signed as-is, so a
/// malicious relay cannot forge or alter a grant for a session it has seen.
fn grant_for(req: &NetEnvelope, identity: &LocalIdentity, body: serde_json::Value) -> NetEnvelope {
    let from = node_id(identity);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let payload = body.to_string().into_bytes();
    let sig = identity
        .sign_signer()
        .map(|s| {
            s.sign(&canonical_bytes(
                &req.session,
                &from,
                &req.from,
                ts,
                &[],
                &payload,
            ))
        })
        .ok();
    NetEnvelope {
        kind: "fragment".into(),
        from,
        to: req.from.clone(),
        session: req.session.clone(),
        seq: 0,
        typ: "grant".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload,
        sig: sig.map(|s| s.as_bytes().to_vec()),
        signer: Some(identity.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(ts),
    }
}

/// Verify an authenticated RES request (signed like a message).
/// The ghost only executes requests from provable senders: freshness,
/// signature, and the trust anchor are all enforced.
pub(crate) fn verify_req(
    env: &NetEnvelope,
    known_peers: &mut HashMap<String, String>,
    now_secs: u64,
) -> bool {
    let (sig, signer, ts) = match (&env.sig, &env.signer, env.ts) {
        (Some(s), Some(pk_hex), Some(t)) if t > 0 => (s.clone(), pk_hex.clone(), t),
        _ => return false,
    };
    if now_secs.abs_diff(ts) > MAX_TS_SKEW_SECS {
        return false; // replay or stale clock
    }
    if let Some(expected_pk) = known_peers.get(&env.from) {
        if expected_pk != &signer {
            return false; // claims to be a known peer, signs as another
        }
    }
    let verifier = match polygone_core::sign::PublicKey::from_hex(&signer) {
        Ok(pk) => polygone_core::sign::Verifier::from_public(pk),
        Err(_) => return false,
    };
    let canonical = canonical_bytes(&env.session, &env.from, &env.to, ts, &[], &env.payload);
    let sig = polygone_core::sign::Signature::from_bytes(&sig);
    if !verifier.verify(&canonical, &sig) {
        return false;
    }
    // TOFU: learn the binding on the first verified request.
    if !known_peers.contains_key(&env.from) {
        known_peers.insert(env.from.clone(), signer);
    }
    true
}

/// Run the task from a RES request inside the systemd sandbox and return
/// the output. Empty input = no execution (grant without output).
/// If the request carries WASM bytes, they run in the wasmi sandbox instead.
/// Runs off the async event loop (spawn_blocking): a long sandboxed run
/// must not freeze message reception.
async fn run_res_task(req: &NetEnvelope) -> Option<String> {
    let body: serde_json::Value = serde_json::from_slice(&req.payload).ok()?;
    if let Some(wasm_b64) = body.get("wasm").and_then(|w| w.as_str()) {
        // WASM execution (Phase 8): decode, run in wasmi, return output.
        let wasm = base64_decode(wasm_b64)?;
        let payload = wasm.to_vec();
        tokio::task::spawn_blocking(move || {
            crate::exec::run_wasm(&payload, std::time::Duration::from_secs(20))
        })
        .await
        .ok()?
        .map_or_else(|e| Some(format!("[erreur wasm] {e}")), Some)
    } else {
        let task = body.get("task")?.as_str()?.to_string();
        if task.trim().is_empty() {
            return None;
        }
        tokio::task::spawn_blocking(move || {
            crate::exec::run_sandboxed(&task, 256, 50, std::time::Duration::from_secs(30))
        })
        .await
        .ok()?
        .map_or_else(|e| Some(format!("[erreur sandbox] {e}")), Some)
    }
}

/// Minimal base64 decoder (no external dep) for WASM transport.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = TABLE.iter().position(|&t| t == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Process one wire line against the session map. Returns `Some` when a
/// session completes (>= 4/7 fragments) AND the ML-DSA signature verifies
/// AND the ciphertext decrypts. Pure logic — no sockets.
///
/// Fail-closed (attaque → rejet) :
/// - enveloppe non adressée à nous, session déjà complétée (anti-replay),
///   signature absente/invalide, horodatage hors fenêtre de fraîcheur,
///   idx de fragment hors 1..=7, plus de 7 fragments, conflit d'identité
///   sur un second KEM, fragments d'un `from` différent du KEM.
/// - plafond de sessions (MAX_SESSIONS) et purge TTL (SESSION_TTL_SECS) :
///   la mémoire du récepteur est bornée.
///
/// `known_peers` (`from` → sign_pk_hex) est l'ancre de confiance : si le
/// `from` est connu et le `signer` ne correspond pas → rejet. Vide =
/// confiance au premier contact (TOFU) — le binding est appris ici même
/// après un message vérifié, et l'appelant le persiste.
fn process_line(
    line: &str,
    identity: &LocalIdentity,
    known_peers: &mut HashMap<String, String>,
    now_secs: u64,
    completed: &mut VecDeque<(String, u64)>,
    sessions: &mut HashMap<String, SessionBuffer>,
) -> Result<Option<(String, Received)>> {
    let raw = line.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let env: NetEnvelope = match serde_json::from_str(raw) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };
    if env.kind != "fragment" {
        return Ok(None);
    }

    // Only accept envelopes addressed to this node.
    if env.to != node_id(identity) {
        return Ok(None);
    }

    // Keep replay ids until no timestamp they carried can still pass freshness.
    // Evicting a still-fresh id on capacity would reopen a completed session.
    completed.retain(|(_, accepted_at)| {
        now_secs.saturating_sub(*accepted_at) <= COMPLETED_RETENTION_SECS
    });

    // Anti-replay: a session that already completed is never accepted again.
    if completed.iter().any(|(sid, _)| sid == &env.session) {
        return Ok(None);
    }

    // Validate the envelope class before allocating session state. Fragments
    // follow their KEM on the same ordered TCP stream, so pre-KEM shares are
    // never needed and must not reserve an unauthenticated slot.
    let parsed_kem = match env.typ.as_str() {
        "kem" => {
            let ct = match kem::KemCiphertext::from_bytes(&env.payload) {
                Ok(ct) => ct,
                Err(_) => return Ok(None),
            };
            let signer = match (&env.signer, env.ts) {
                (Some(signer), Some(ts)) if ts > 0 && now_secs.abs_diff(ts) <= MAX_TS_SKEW_SECS => {
                    signer
                }
                _ => return Ok(None),
            };
            if known_peers
                .get(&env.from)
                .is_some_and(|expected| expected != signer)
                || polygone_core::sign::PublicKey::from_hex(signer).is_err()
                || env.sig.is_none()
            {
                return Ok(None);
            }
            Some(ct)
        }
        "frag" if (1..=7).contains(&env.idx) => None,
        _ => return Ok(None),
    };

    // A session belongs to one sender: key = from + session.
    let skey = format!("{}|{}", env.from, env.session);

    if env.typ == "frag"
        && !sessions.get(&skey).is_some_and(|buf| {
            buf.kem_ct.is_some()
                && buf
                    .ts
                    .is_some_and(|ts| now_secs.abs_diff(ts) <= MAX_TS_SKEW_SECS)
        })
    {
        return Ok(None);
    }

    // Purge before enforcing the cap: otherwise a full table of expired
    // sessions would reject every new session indefinitely.
    if sessions.len().is_multiple_of(256)
        || sessions.len() >= MAX_SESSIONS
        || completed.len().saturating_add(sessions.len()) >= MAX_COMPLETED
    {
        sessions.retain(|_, b| now_secs.saturating_sub(b.touched) < SESSION_TTL_SECS);
    }
    // Fail closed beyond the session cap after reclaiming expired entries.
    if sessions.len() >= MAX_SESSIONS && !sessions.contains_key(&skey) {
        return Ok(None);
    }
    // Reserve one anti-replay slot for every incomplete session. When the
    // bounded cache is full, refuse new sessions instead of evicting fresh ids.
    if !sessions.contains_key(&skey)
        && completed.len().saturating_add(sessions.len()) >= MAX_COMPLETED
    {
        return Ok(None);
    }

    // A second KEM for the same key must come from the SAME identity.
    let buf = sessions.entry(skey.clone()).or_default();
    match env.typ.as_str() {
        "kem" => {
            if buf.kem_ct.is_some() {
                // Conflicting second KEM (different signer/from) → session
                // confusion attack: reject the whole session.
                if buf.signer.as_ref() != env.signer.as_ref() || buf.from != env.from {
                    return Ok(None);
                }
                // An identical KEM retransmission is not new activity and may
                // not pin an incomplete slot past its original timeout.
                return Ok(None);
            }
            buf.touched = now_secs;
            buf.kem_ct = parsed_kem;
            buf.sig = env.sig.clone();
            buf.signer = env.signer.clone();
            buf.name_ct = env.name_ct.clone();
            buf.ts = env.ts;
            buf.from = env.from.clone();
            buf.to = env.to.clone();
        }
        "frag" => {
            // Never buffer more than the total — bounds combinations4.
            if buf.fragments.len() >= 7 {
                return Ok(None);
            }
            // Fragments must belong to the same sender as the KEM.
            if buf.kem_ct.is_some() && env.from != buf.from {
                return Ok(None);
            }
            // Duplicate index → drop (a second idx=1 corrupts reconstruction).
            if buf.seen_idx.contains(&env.idx) {
                return Ok(None);
            }
            buf.seen_idx.push(env.idx);
            buf.fragments.push(shamir::Fragment {
                id: shamir::FragmentId(env.idx),
                data: env.payload,
            });
            // Only a new, valid fragment extends the inactivity timeout.
            buf.touched = now_secs;
        }
        _ => {}
    }

    if buf.kem_ct.is_none() || buf.fragments.len() < 4 {
        return Ok(None);
    }

    // ── Completion: freshness, verify, decrypt. Fail closed on either. ──
    let kem_ct = buf.kem_ct.clone().expect("checked");
    let sig = match (buf.sig.clone(), buf.signer.clone()) {
        (Some(s), Some(pk_hex)) => (s, pk_hex),
        _ => return Ok(None), // no signature → not from anyone provable
    };
    // Freshness: the signed timestamp must be within ± MAX_TS_SKEW_SECS.
    let ts = match buf.ts {
        Some(t) if t > 0 => t,
        _ => {
            sessions.remove(&skey);
            return Ok(None);
        }
    };
    if now_secs.abs_diff(ts) > MAX_TS_SKEW_SECS {
        sessions.remove(&skey);
        return Ok(None); // replay or clock skew — refuse
    }
    // Trust anchor: if we know this peer's signing key, it must match.
    if let Some(expected_pk) = known_peers.get(&buf.from) {
        if expected_pk != &sig.1 {
            sessions.remove(&skey);
            return Ok(None); // claims to be a known peer, signs as another
        }
    }
    let verifier = match polygone_core::sign::PublicKey::from_hex(&sig.1) {
        Ok(pk) => polygone_core::sign::Verifier::from_public(pk),
        Err(_) => {
            sessions.remove(&skey);
            return Ok(None);
        }
    };

    let shared = match kem::decapsulate(&identity.kem_secret_key()?, &kem_ct) {
        Ok(shared) => shared,
        Err(_) => {
            sessions.remove(&skey);
            return Ok(None);
        }
    };
    let key = symmetric::SessionKey::derive_from_secret(&shared);

    // Bounded by construction (≤7 fragments), kept as a hard guard: the
    // worst case is C(7,4) = 35 combinations, never C(n+1,5).
    if buf.fragments.len() > 7 {
        return Ok(None);
    }
    // Shamir reconstruction is deterministic: any 4-of-7 fragments rebuild
    // the same secret. Try the available 4-subsets until the signature
    // verifies (handles one corrupted/forged fragment), then decrypt.
    let sig_bytes = polygone_core::sign::Signature::from_bytes(&sig.0);
    let mut ciphertext = None;
    for combo in combinations4(&buf.fragments) {
        let cand = match shamir::reconstruct(&combo, 4) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let canonical = canonical_bytes(
            &env.session,
            &buf.from,
            &buf.to,
            ts,
            kem_ct.as_bytes(),
            &cand,
        );
        if verifier.verify(&canonical, &sig_bytes) {
            ciphertext = Some(cand);
            break;
        }
    }
    let ciphertext = match ciphertext {
        Some(c) => c,
        None => {
            // All 7 indices have arrived, so no later subset can rescue this
            // signature; release the slot instead of retaining forged state.
            if buf.fragments.len() == 7 {
                sessions.remove(&skey);
            }
            return Ok(None); // no 4-subset verifies → forged/corrupt
        }
    };
    let plain = match symmetric::decrypt(&ciphertext, &key) {
        Ok(p) => p,
        Err(_) => {
            sessions.remove(&skey);
            return Ok(None);
        }
    };

    let session = env.session.clone();
    let sender = buf.from.clone();
    let name_ct = buf.name_ct.clone();
    // These errors are terminal: the signed ciphertext has already verified,
    // so no later fragment can make its filename or Drive header valid.
    // Reject only this envelope, release its slot, and keep listening.
    let received_result = (|| -> std::result::Result<Received, anyhow::Error> {
        let received = match name_ct {
            Some(ct) => {
                // Decrypt the file name out-of-band. A bad name tag or encoding
                // is rejected rather than silently saving under a fallback name.
                let name = String::from_utf8(symmetric::decrypt(&ct, &key)?)?;
                let name = crate::drive::safe_filename(&name)?;
                if crate::drive::DriveFile::is_network_chunk(&plain) {
                    let chunk = crate::drive::DriveFile::decode_network_chunk(&plain)?;
                    if chunk.filename != name {
                        anyhow::bail!("nom du bloc Drive incohérent avec l’enveloppe");
                    }
                    Received::DriveChunk {
                        sender: sender.clone(),
                        chunk,
                    }
                } else {
                    Received::File { name, bytes: plain }
                }
            }
            None => Received::Message(String::from_utf8_lossy(&plain).to_string()),
        };
        Ok(received)
    })();
    let received = match received_result {
        Ok(received) => received,
        Err(_) => {
            sessions.remove(&skey);
            return Ok(None);
        }
    };
    // Admission reserves this slot; fail closed if state was constructed
    // outside that invariant rather than outputting an untracked session.
    if completed.len() >= MAX_COMPLETED {
        sessions.remove(&skey);
        return Ok(None);
    }
    // TOFU: learn the binding from → sign_pk on the first verified contact.
    known_peers.entry(sender).or_insert_with(|| sig.1.clone());
    sessions.remove(&skey);
    completed.push_back((session.clone(), now_secs));
    Ok(Some((session, received)))
}

/// All 4-element subsets of the buffered fragments (max C(7,4) = 35).
fn combinations4(fragments: &[shamir::Fragment]) -> Vec<Vec<shamir::Fragment>> {
    let n = fragments.len();
    let mut out = Vec::new();
    if n < 4 {
        return out;
    }
    let mut idx: Vec<usize> = (0..4).collect();
    loop {
        out.push(idx.iter().map(|&i| fragments[i].clone()).collect());
        // Find the rightmost index that can still advance.
        let mut i = 3usize;
        while i > 0 && idx[i] == n - 4 + i {
            i -= 1;
        }
        if idx[i] == n - 4 + i {
            break; // no index can advance — exhausted
        }
        idx[i] += 1;
        for j in i + 1..4 {
            idx[j] = idx[j - 1] + 1;
        }
    }
    out
}

/// Listen for incoming messages through the relay. Blocks until interrupted.
/// Reconstructs and decrypts each session as soon as 4/7 fragments arrive.
/// File transfers are saved to `~/.polygone/received/`. When `compute` is
/// set, RES requests (`req` envelopes) get a `grant` reply (ghost node).
pub async fn receive_network(relay: &str, identity: &LocalIdentity, compute: bool) -> Result<()> {
    // Refuse to connect or process trusted traffic until the store has loaded
    // successfully (including its initialization/migration marker).
    let mut known_peers = load_peers()?;
    let stream = connect(relay, identity).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    // Keep partial, bounded bytes across periodic timer ticks; the capped
    // reader appends synchronously after each cancellation-safe `fill_buf`.
    let mut line_bytes = Vec::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    let mut pending_drive_files = PendingDriveFiles::default();
    let mut cleanup = tokio::time::interval(std::time::Duration::from_secs(30));
    cleanup.tick().await;
    println!(
        "⬡ en écoute via relay {} — node {}",
        relay,
        node_id(identity)
    );
    if !known_peers.is_empty() {
        println!(
            "  ⬡ ancre de confiance : {} pair(s) connu(s) — empreintes à vérifier hors-ligne :",
            known_peers.len()
        );
        for (node, pk_hex) in known_peers.iter() {
            let fp: String = pk_hex.chars().take(16).collect();
            println!("      · {node} → {fp}…");
        }
    }
    if compute {
        println!("  ⬡ RES : prêt de compute actif (les requêtes seront accordées)");
    }
    println!("  (Ctrl-C pour arrêter)\n");

    loop {
        let n = tokio::select! {
            result = read_line_capped(&mut reader, &mut line_bytes, MAX_RELAY_LINE_BYTES) => result?,
            _ = cleanup.tick() => {
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                pending_drive_files.expire_stale(now_secs);
                sessions.retain(|_, buffer| {
                    now_secs.saturating_sub(buffer.touched) < SESSION_TTL_SECS
                });
                continue;
            }
        };
        if n == 0 {
            println!("relay déconnecté.");
            return Ok(());
        }
        let line = String::from_utf8_lossy(&line_bytes).into_owned();
        line_bytes.clear();
        // The relay acknowledges the HELLO: a denied slot means another
        // connection owns this node_id — the traffic would go nowhere.
        if line.starts_with("HELLO_DENIED") {
            println!(
                "✖ ce node_id est déjà connecté au relay (slot pris par une autre connexion)."
            );
            println!("  Si c'est un ancien process, fermez-le et réessayez.");
            return Ok(());
        }
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // RES compute requests: answer with a grant (ghost node).
        // Only answer requests addressed to this node, and only if the
        // request is AUTHENTICATED (signed req, fresh timestamp). The
        // borrower's reputation is recorded for real on failure.
        if let Ok(env) = serde_json::from_str::<NetEnvelope>(line.trim()) {
            if env.typ == "req" && compute && env.to == node_id(identity) {
                let previous = known_peers.clone();
                let req_ok = verify_req(&env, &mut known_peers, now_secs);
                if req_ok {
                    persist_peer_changes_at(&mut known_peers, &previous, &peers_path())?;
                }
                if !req_ok {
                    // Fail-closed: refuse, and record the failure so the
                    // reputation gate eventually refuses this borrower.
                    crate::reputation::ReputationTable::load().record(&env.from, false);
                    let mut body = grant_payload(identity);
                    body["ok"] = serde_json::Value::Bool(false);
                    body["refus"] = serde_json::Value::String("requête non authentifiée".into());
                    let grant = grant_for(&env, identity, body);
                    let _ = writer
                        .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                        .await;
                    continue;
                }
                // Reputation gate: a borrower known to fail is refused.
                let table = crate::reputation::ReputationTable::load();
                if let Some(rep) = table.nodes.get(&env.from) {
                    if rep.fail >= 3 && rep.score() < 30 {
                        let mut body = grant_payload(identity);
                        body["ok"] = serde_json::Value::Bool(false);
                        body["refus"] = serde_json::Value::String("réputation insuffisante".into());
                        let grant = grant_for(&env, identity, body);
                        let _ = writer
                            .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                            .await;
                        continue;
                    }
                }
                let mut body = grant_payload(identity);
                // Execute the task in the sandbox (RES execution layer),
                // off the event loop. Record success in the reputation.
                if let Some(output) = run_res_task(&env).await {
                    body["output"] = serde_json::Value::String(output);
                    crate::reputation::ReputationTable::load().record(&env.from, true);
                }
                let grant = grant_for(&env, identity, body);
                let _ = writer
                    .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                    .await;
                println!("⬡ RES : compute accordé à {}", env.from);
                continue;
            }
        }

        let n_before = known_peers.len();
        match process_line_with_persistence(
            &line,
            identity,
            &mut known_peers,
            now_secs,
            &mut completed,
            &mut sessions,
            &peers_path(),
        )? {
            Some((session, Received::Message(text))) => {
                // The wrapper has durably committed a newly learned pin before
                // this payload can be shown to the user.
                announce_new_peer(&known_peers, n_before);
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("⬡ message reçu (session {session} · 4/7 fragments)");
                println!();
                println!("{text}");
                println!();
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            Some((session, Received::File { name, bytes })) => {
                // The wrapper has durably committed a newly learned pin before
                // this payload can be written to disk.
                announce_new_peer(&known_peers, n_before);
                let dir = received_dir()?;
                let (_, path) = crate::drive::save_received_in(&dir, &name, &bytes)?;
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!(
                    "⬡ fichier reçu : {} ({} octets · session {session} · 4/7 fragments)",
                    path.display(),
                    bytes.len()
                );
                println!("  BLAKE3 {}", blake3::hash(&bytes).to_hex());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            Some((session, Received::DriveChunk { sender, chunk })) => {
                announce_new_peer(&known_peers, n_before);
                match pending_drive_files.accept(sender, chunk, now_secs) {
                    Ok(Some((name, bytes))) => {
                        let dir = received_dir()?;
                        let (_, path) = crate::drive::save_received_in(&dir, &name, &bytes)?;
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!(
                            "⬡ fichier reçu : {} ({} octets · session {session} · blocs Drive réassemblés)",
                            path.display(),
                            bytes.len()
                        );
                        println!("  BLAKE3 {}", blake3::hash(&bytes).to_hex());
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    }
                    Ok(None) => {}
                    Err(error) => log::warn!("transfert Drive réseau rejeté : {error}"),
                }
            }
            None => {}
        }
    }
}

/// Directory where received files land.
fn received_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let dir = home.join(".polygone").join("received");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ─────────────────────────────────────────────────────────────────────────────
// Polygone Hide — SOCKS5 proxy through the blind relay (HIDE-SPEC.md Phase 1).
//
// The `hide` service reuses the existing transport (NDJSON envelopes routed by
// the blind relay on `to`) with a dedicated envelope type `stream`:
//
//   client (SOCKS5 127.0.0.1:9050)          relay             exit node
//      │  CONNECT host:port                      │                 │
//      │  req (KEM_CT + AES-GCM{host,port})      │                 │  decapsulate →
//      │  ──────────────────────────────────────►│  route on `to`  │  TcpStream::connect
//      │  grant (AES-GCM{ok})                    │                 │
//      │  ◄──────────────────────────────────────│  ◄──────────────│
//      │  stream (AES-GCM{octets})               │                 │  application data
//      │  ◄─────────────────────────────────────►│  ◄─────────────►│
//
// The relay only reads kind/to/session — the payload is opaque bytes. The
// exit node sees the real destination (like a Tor exit, documented). The
// content is end-to-end encrypted with the ML-KEM session key.
// ─────────────────────────────────────────────────────────────────────────────

/// Max application bytes carried by one `stream` envelope. Bounded: the relay
/// caps a line at 64 KiB (MAX_LINE), and a GCM ciphertext adds an auth tag.
pub const STREAM_CHUNK: usize = 16 * 1024;

/// Build a `stream` envelope carrying `payload` (already encrypted bytes).
pub fn stream_envelope(session: &str, from: &str, to: &str, payload: &[u8]) -> NetEnvelope {
    NetEnvelope {
        kind: "fragment".into(),
        from: from.to_string(),
        to: to.to_string(),
        session: session.to_string(),
        seq: 0,
        typ: "stream".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: payload.to_vec(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    }
}

/// Build a `grant` envelope — the exit node's encrypted answer to a hide
/// `req`. Only the requester (who holds the session key) can decrypt it;
/// the relay sees opaque bytes and cannot forge it.
pub fn grant_envelope(session: &str, from: &str, to: &str, payload: &[u8]) -> NetEnvelope {
    NetEnvelope {
        kind: "fragment".into(),
        from: from.to_string(),
        to: to.to_string(),
        session: session.to_string(),
        seq: 0,
        typ: "grant".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: payload.to_vec(),
        sig: None,
        signer: None,
        name_ct: None,
        ts: None,
    }
}

/// Serialize an envelope to the NDJSON wire line (helper used by hide tasks).
pub fn envelope_line(env: &NetEnvelope) -> Result<String> {
    Ok(format!("{}\n", serde_json::to_string(env)?))
}

/// The channel a hide client opens to an exit node. After `hide_establish`
/// succeeds, the caller owns the split relay stream and the session key —
/// `reader` carries the exit node's replies (grant + stream envelopes),
/// `writer` carries our stream envelopes toward the exit node.
pub struct HideChannel {
    /// The relay-side session id (routing + correlation).
    pub session: String,
    /// The AES-GCM session key derived from the ML-KEM shared secret.
    pub key: symmetric::SessionKey,
    /// Reads envelopes coming back from the exit node (grant + stream).
    pub reader: tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
    /// Writes our stream envelopes toward the exit node.
    pub writer: tokio::net::tcp::OwnedWriteHalf,
}

/// Open a hide tunnel to `exit_node` through the relay: send the encrypted
/// CONNECT request (`{"action":"hide","host":...,"port":...}`), wait for the
/// signed grant, and return the split channel. Fail-closed on any mismatch.
pub async fn hide_establish(
    relay: &str,
    exit_node: &str,
    exit_pk: &kem::KemPublicKey,
    identity: &LocalIdentity,
    host: &str,
    port: u16,
    timeout: std::time::Duration,
) -> Result<HideChannel> {
    let mut stream = connect(relay, identity).await?;
    let from = node_id(identity);
    let session = session_id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Encrypt the destination: KEM encapsulate → session key → AES-GCM.
    // The relay sees only `kem_ct` + ciphertext bytes — never host/port.
    let (kem_ct, shared) = kem::encapsulate(exit_pk)?;
    let key = symmetric::SessionKey::derive_from_secret(&shared);
    let body = serde_json::json!({"action": "hide", "host": host, "port": port})
        .to_string()
        .into_bytes();
    let ct = symmetric::encrypt(&body, &key)?;
    let mut payload = Vec::with_capacity(kem_ct.as_bytes().len() + ct.len());
    payload.extend_from_slice(kem_ct.as_bytes());
    payload.extend_from_slice(&ct);

    // The request is signed (like RES requests): the exit node verifies who
    // asks for a tunnel before opening a connection on its behalf.
    let req_sig = identity.sign_signer()?.sign(&canonical_bytes(
        &session,
        &from,
        exit_node,
        ts,
        &[],
        &payload,
    ));
    let req = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: exit_node.to_string(),
        session: session.clone(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload,
        sig: Some(req_sig.as_bytes().to_vec()),
        signer: Some(identity.sign_pk_hex.clone()),
        name_ct: None,
        ts: Some(ts),
    };
    stream
        .write_all(format!("{}\n", serde_json::to_string(&req)?).as_bytes())
        .await?;
    stream.flush().await?;

    // Wait for the grant on the same connection. A grant is only accepted if
    // it answers OUR request: addressed to us, same session, from the exit
    // node, and its payload decrypts with our session key.
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        line.clear();
        let n = tokio::time::timeout(
            timeout,
            read_line_capped(&mut reader, &mut line, MAX_RELAY_LINE_BYTES),
        )
        .await
        .unwrap_or(Ok(0))?;
        if n == 0 {
            break;
        }
        let env: NetEnvelope = match serde_json::from_slice(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if env.typ == "grant" && env.session == session && env.to == from {
            let ok = symmetric::decrypt(&env.payload, &key)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .and_then(|v| v.get("ok").and_then(|o| o.as_bool()))
                .unwrap_or(false);
            if !ok {
                anyhow::bail!("exit node a refusé la connexion hide");
            }
            let (r, w) = reader.into_inner().into_split();
            return Ok(HideChannel {
                session,
                key,
                reader: BufReader::new(r),
                writer: w,
            });
        }
    }
    anyhow::bail!("aucun grant reçu — le nœud de sortie est-il en `ecouter --hide` ?")
}

/// Decrypt a `stream` envelope payload. Returns `Ok(None)` for an empty
/// payload = end-of-stream marker (peer closed its side).
pub fn hide_decrypt_chunk(payload: &[u8], key: &symmetric::SessionKey) -> Result<Option<Vec<u8>>> {
    if payload.is_empty() {
        return Ok(None);
    }
    Ok(Some(symmetric::decrypt(payload, key)?))
}

/// Encrypt application bytes into a `stream` envelope payload.
pub fn hide_encrypt_chunk(chunk: &[u8], key: &symmetric::SessionKey) -> Result<Vec<u8>> {
    Ok(symmetric::encrypt(chunk, key)?)
}

#[cfg(test)]
mod tests;
