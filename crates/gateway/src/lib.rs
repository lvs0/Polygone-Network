//! Polygone Gateway — HTTP API + site statique + appairage.
//!
//! Le crate est découpé en couches indépendantes, chacune testable sans
//! socket :
//!
//! - [`http`] : requête/réponse et encodage HTTP minimal ;
//! - [`routes`] : routage pur `HttpRequest -> HttpResponse` ;
//! - [`status`] : état réseau réellement mesuré (`active`, médiane de latence) ;
//! - [`static_files`] : service du site, avec refus des traversées de chemin.
//!
//! Ce fichier ne contient que le câblage : un socket, un parseur de requête, et
//! le [`Gateway`] qui porte l'état partagé.
//!
//! Endpoints servis (voir [`routes`]) :
//!   GET  /                       → site officiel
//!   GET  /api/v1/network/status  → état du réseau
//!   GET  /api/v1/services        → état déclaré des services
//!   POST /api/v1/pair/request    → demande d'appairage
//!   GET  /api/v1/pair/qr/:token  → deep link d'appairage
//!   POST /api/v1/pair/accept     → acceptation côté client
//!   POST /api/v1/nodes/heartbeat → heartbeat signé, `last_seen` persisté
//!   POST /api/v1/nodes/ping      → challenge de mesure, signé
//!   POST /api/v1/nodes/pong      → RTT mesuré, challenge à usage unique
//!   GET  /api/v1/node/:id        → info d'un nœud

pub mod http;
pub mod routes;
pub mod static_files;
pub mod status;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, bail};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

pub use http::{HttpRequest, HttpResponse};
pub use status::{NetworkStatus, NodeInfo};

/// En-têtes acceptés au maximum avant de refuser la requête.
const MAX_HEAD_BYTES: usize = 16 * 1024;
/// Corps accepté au maximum (les appairages sont de petits JSON).
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// Bound accepted sockets so a local/network client cannot spawn unlimited tasks.
const MAX_CONNECTIONS: usize = 128;
/// One absolute deadline for the full request head and declared body.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const RESPONSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const MAX_REQUEST_TARGET_BYTES: usize = 2048;
/// A pairing bearer token expires after five minutes.
pub const PAIR_TOKEN_TTL_SECS: u64 = 300;
/// Bound the number of pending bearer tokens held in process memory.
pub const MAX_PENDING_PAIRINGS: usize = 4096;

/// Demande d'appairage envoyée par un appareil.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairRequest {
    pub device_name: String,
    /// ML-KEM-1024 encapsulation key; relay node IDs are its first 16 hex chars.
    pub kem_public_key_hex: String,
    /// ML-DSA-65 verification key used to authenticate this device.
    pub sign_public_key_hex: String,
}

/// Jeton d'appairage en attente d'acceptation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairToken {
    pub token: String,
    pub device_name: String,
    pub kem_public_key_hex: String,
    pub sign_public_key_hex: String,
    pub created_at: u64,
}

/// Bearer-token-only acknowledgement of an existing pairing request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairAcceptRequest {
    pub token: String,
}

/// Ping émis, en attente de pong. Transitoire : jamais écrit sur disque.
#[derive(Clone, Debug)]
pub struct PendingPing {
    pub challenge: String,
    pub issued_at: u64,
}

/// État partagé du gateway : le site à servir et les nœuds/appairages connus.
#[derive(Clone)]
pub struct Gateway {
    pub site_dir: PathBuf,
    pub pairs: Arc<RwLock<HashMap<String, PairToken>>>,
    pub nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
    /// Challenges de ping en attente : à usage unique, expirés par le handler.
    pub pings: Arc<RwLock<HashMap<String, PendingPing>>>,
    /// Fichier où les nœuds survivent au redémarrage. `None` = mémoire seule
    /// (tests et usage jetable), jamais une persistance implicite ailleurs.
    pub nodes_file: Option<PathBuf>,
    /// Sérialise les écritures du fichier : deux heartbeats concurrents ne
    /// doivent pas produire un fichier entrelacé.
    persist_lock: Arc<tokio::sync::Mutex<()>>,
    /// Instant de démarrage, en secondes Unix : sert à un uptime mesuré.
    pub started_at: u64,
}

impl Gateway {
    pub fn new(site_dir: PathBuf) -> Self {
        Self::with_nodes_file(site_dir, None)
    }

    /// Construit un gateway qui recharge `nodes_file` au démarrage et
    /// l'écrit après chaque heartbeat. Un fichier illisible ne fait pas
    /// tomber la gateway : il repart d'une table vide et le dit dans le log.
    pub fn with_nodes_file(site_dir: PathBuf, nodes_file: Option<PathBuf>) -> Self {
        let nodes = nodes_file
            .as_deref()
            .and_then(|path| match std::fs::read(path) {
                Ok(bytes) => match serde_json::from_slice::<HashMap<String, NodeInfo>>(&bytes) {
                    Ok(nodes) => Some(nodes),
                    Err(error) => {
                        log::warn!(
                            "gateway : {} illisible ({error}) — table de nœuds vide",
                            path.display()
                        );
                        None
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    log::warn!(
                        "gateway : lecture de {} impossible ({error}) — table vide",
                        path.display()
                    );
                    None
                }
            })
            .unwrap_or_default();

        Self {
            site_dir,
            pairs: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            pings: Arc::new(RwLock::new(HashMap::new())),
            nodes_file,
            persist_lock: Arc::new(tokio::sync::Mutex::new(())),
            started_at: now_secs(),
        }
    }

    /// Écrit la table des nœuds de façon atomique (temporaire puis `rename`).
    /// Sans fichier configuré, ne fait rien : la mémoire est l'état.
    pub async fn persist_nodes(&self, nodes: &HashMap<String, NodeInfo>) {
        let Some(path) = self.nodes_file.as_deref() else {
            return;
        };
        let _guard = self.persist_lock.lock().await;
        let body = match serde_json::to_vec_pretty(nodes) {
            Ok(body) => body,
            Err(error) => {
                log::error!("gateway : sérialisation des nœuds impossible : {error}");
                return;
            }
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            log::error!(
                "gateway : création de {} impossible : {error}",
                parent.display()
            );
            return;
        }
        let temporary = path.with_extension("json.tmp");
        if let Err(error) = std::fs::write(&temporary, body) {
            log::error!(
                "gateway : écriture de {} impossible : {error}",
                temporary.display()
            );
            return;
        }
        if let Err(error) = std::fs::rename(&temporary, path) {
            log::error!(
                "gateway : remplacement de {} impossible : {error}",
                path.display()
            );
        }
    }

    /// Lie `addr` puis sert jusqu'à la fin du process.
    pub async fn run(addr: &str, site_dir: PathBuf) -> Result<()> {
        let address = validate_loopback_addr(addr)?;
        let nodes_file = default_nodes_path();
        match &nodes_file {
            Some(path) => info!("⬡ gateway : heartbeat persisté dans {}", path.display()),
            None => log::warn!("⬡ gateway : aucun HOME — heartbeat en mémoire seulement"),
        }
        let listener = TcpListener::bind(address).await?;
        info!("⬡ gateway : HTTP local sur {address}");
        Self::serve_with_nodes(listener, site_dir, nodes_file).await
    }

    /// Sert sur une socket déjà liée. Séparer le bind du service évite les
    /// fenêtres de course lors des tests et permet à l'installateur de réserver
    /// le port avant de démarrer les tâches asynchrones.
    pub async fn serve(listener: TcpListener, site_dir: PathBuf) -> Result<()> {
        Self::serve_with_nodes(listener, site_dir, None).await
    }

    /// Sert avec persistance explicite des nœuds. Séparé de [`Gateway::serve`]
    /// pour que les tests restent hermétiques : aucun fichier `$HOME` écrit
    /// par accident depuis une suite de tests.
    pub async fn serve_with_nodes(
        listener: TcpListener,
        site_dir: PathBuf,
        nodes_file: Option<PathBuf>,
    ) -> Result<()> {
        let address = listener.local_addr()?;
        if !address.ip().is_loopback() {
            bail!(
                "Gateway HTTP sans TLS/authentification mutuelle : écoute publique refusée ({address}); utilisez une adresse loopback"
            );
        }
        let gateway = Self::with_nodes_file(site_dir, nodes_file);
        let permits = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(err) => {
                    error!("gateway : erreur accept : {err}");
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            let permit = match permits.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // Keep the listener responsive under connection floods.
                    drop(stream);
                    continue;
                }
            };
            let gateway = gateway.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(err) = gateway.handle(stream).await {
                    error!("gateway : connexion {peer} interrompue : {err}");
                }
            });
        }
    }

    /// Sert une connexion avec des délais absolus de lecture et d'écriture.
    async fn handle(&self, mut stream: TcpStream) -> Result<()> {
        let request = match tokio::time::timeout(REQUEST_TIMEOUT, read_request(&mut stream)).await {
            Err(_) => Some(HttpResponse::text(408, "délai de requête dépassé").cors()),
            Ok(Err(RequestReadError::Malformed)) => {
                Some(HttpResponse::text(400, "requête HTTP mal formée").cors())
            }
            Ok(Err(RequestReadError::HeadersTooLarge)) => {
                Some(HttpResponse::text(431, "en-têtes trop volumineux").cors())
            }
            Ok(Err(RequestReadError::BodyTooLarge)) => {
                Some(HttpResponse::text(413, "corps trop volumineux").cors())
            }
            Ok(Err(RequestReadError::Io(err))) => return Err(err.into()),
            Ok(Ok(None)) => return Ok(()),
            Ok(Ok(Some(request))) => Some(self.route(request).await),
        };

        if let Some(response) = request {
            tokio::time::timeout(RESPONSE_TIMEOUT, async {
                stream.write_all(&response.encode()).await?;
                stream.flush().await
            })
            .await
            .map_err(|_| anyhow::anyhow!("délai d'écriture HTTP dépassé"))??;
        }
        Ok(())
    }
}

/// Le Gateway est un service de développement/local tant qu'il n'a pas TLS
/// et authentification mutuelle. Refuser les binds publics même si l'adresse
/// a été fournie explicitement par variable d'environnement.
fn validate_loopback_addr(value: &str) -> Result<std::net::SocketAddr> {
    let address: std::net::SocketAddr = value
        .parse()
        .map_err(|_| anyhow::anyhow!("adresse Gateway invalide : {value}"))?;
    if !address.ip().is_loopback() {
        bail!(
            "Gateway HTTP sans TLS/authentification mutuelle : écoute publique refusée ({address}); utilisez 127.0.0.1 ou [::1]"
        );
    }
    Ok(address)
}

#[derive(Debug)]
enum RequestReadError {
    Malformed,
    HeadersTooLarge,
    BodyTooLarge,
    Io(std::io::Error),
}

impl From<std::io::Error> for RequestReadError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Lit une requête HTTP/1.1 complète. Les formes ambiguës (Content-Length
/// dupliqué, Transfer-Encoding, lignes pliées) sont refusées plutôt que devinées.
async fn read_request(
    stream: &mut TcpStream,
) -> std::result::Result<Option<HttpRequest>, RequestReadError> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];

    let head_end = loop {
        if let Some(position) = find_head_end(&buffer) {
            if position + 4 > MAX_HEAD_BYTES {
                return Err(RequestReadError::HeadersTooLarge);
            }
            break position;
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(RequestReadError::HeadersTooLarge);
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return if buffer.is_empty() {
                Ok(None)
            } else {
                Err(RequestReadError::Malformed)
            };
        }
        buffer.extend_from_slice(&chunk[..read]);
    };

    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| RequestReadError::Malformed)?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or(RequestReadError::Malformed)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or(RequestReadError::Malformed)?;
    let target = parts.next().ok_or(RequestReadError::Malformed)?;
    let version = parts.next().ok_or(RequestReadError::Malformed)?;
    if parts.next().is_some()
        || version != "HTTP/1.1"
        || method.is_empty()
        || !method.bytes().all(|byte| byte.is_ascii_uppercase())
        || !target.starts_with('/')
        || target.starts_with("//")
        || target.len() > MAX_REQUEST_TARGET_BYTES
        || target
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
        || target.contains('#')
    {
        return Err(RequestReadError::Malformed);
    }

    let mut content_length = None;
    let mut host_count = 0usize;
    for line in lines {
        if line.is_empty() || line.starts_with([' ', '\t']) {
            return Err(RequestReadError::Malformed);
        }
        let (name, value) = line.split_once(':').ok_or(RequestReadError::Malformed)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || value
                .bytes()
                .any(|byte| (byte < 0x20 && byte != b'\t') || byte == 0x7f)
        {
            return Err(RequestReadError::Malformed);
        }
        if name.eq_ignore_ascii_case("host") {
            host_count += 1;
            if value.trim().is_empty() {
                return Err(RequestReadError::Malformed);
            }
        } else if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(RequestReadError::Malformed);
            }
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| RequestReadError::Malformed)?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            // Chunked bodies are not implemented. Never interpret the same
            // bytes differently from a proxy in front of this server.
            return Err(RequestReadError::Malformed);
        }
    }
    if host_count != 1 {
        return Err(RequestReadError::Malformed);
    }

    let content_length = content_length.unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err(RequestReadError::BodyTooLarge);
    }

    let path = target.split('?').next().unwrap_or("/").to_string();
    let body_start = head_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(RequestReadError::Malformed);
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(Some(HttpRequest {
        method: method.to_string(),
        path,
        body,
    }))
}

/// Position du `\r\n\r\n` qui termine les en-têtes.
fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Chemin par défaut du fichier de nœuds : `$POLYGONE_GATEWAY_NODES` puis
/// `$HOME/.polygone/nodes.json`. Sans HOME, aucune persistance implicite.
pub fn default_nodes_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("POLYGONE_GATEWAY_NODES") {
        return Some(PathBuf::from(explicit));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".polygone").join("nodes.json"))
}

/// Secondes Unix courantes.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 256 bits aléatoires cryptographiquement sûrs, encodés en hexadécimal.
/// Le token est un secret porteur de capacité ; huit caractères ne suffisent pas.
pub fn uuid_short() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_accepts_only_loopback_bind_addresses() {
        assert!(validate_loopback_addr("127.0.0.1:8088").is_ok());
        assert!(validate_loopback_addr("[::1]:8088").is_ok());
        assert!(validate_loopback_addr("0.0.0.0:8088").is_err());
        assert!(validate_loopback_addr("192.0.2.10:8088").is_err());
        assert!(validate_loopback_addr("localhost:8088").is_err());
    }

    #[test]
    fn head_end_is_found_after_headers() {
        let raw = b"GET / HTTP/1.1\r\nHost: x\r\n\r\nreste";
        // Index du premier `\r` de la ligne vide : 23.
        assert_eq!(find_head_end(raw), Some(23));
        assert_eq!(&raw[23 + 4..], b"reste");
    }

    #[test]
    fn head_end_is_none_without_blank_line() {
        assert_eq!(find_head_end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);
    }

    #[test]
    fn now_secs_is_plausible() {
        // 2026-01-01 : si l'horloge est antérieure, quelque chose cloche.
        assert!(now_secs() > 1_767_225_600);
    }

    #[test]
    fn pairing_tokens_are_256_bits_of_hexadecimal_entropy() {
        let token = uuid_short();
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(token, uuid_short());
    }
}
