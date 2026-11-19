//! Transports d'exécution.
//!
//! Deux implémentations du même contrat [`crate::engine::Transport`] :
//!
//! - [`MemoryTransport`] : nœuds en mémoire, utilisé par les tests et la
//!   démonstration. Il exécute réellement [`polygone_core::task::TaskKind`] et
//!   signe réellement le résultat.
//! - [`TcpTransport`] : un socket TCP réel, NDJSON, une requête par connexion.
//!   Le pair répond avec ses capacités ou exécute la tâche et renvoie un
//!   [`polygone_core::task::TaskResult`] signé.
//!
//! # Limites assumées
//!
//! - Pas de TLS : ce prototype est destiné au loopback et au LAN de confiance.
//!   Un nœud distant voit la tâche en clair.
//! - La **demande** n'est pas authentifiée ; c'est le **résultat** qui l'est
//!   (ML-DSA-65, `NodeId` dérivé de la clé). Un serveur ne doit donc pas
//!   accorder de permission implicite à l'appelant.
//! - Une seule requête par connexion, pas de reprise ni de streaming.
//! - Le plafond de connexions est local au processus ; il n'est pas un quota
//!   noyau.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use polygone_core::sign::{generate_keypair, node_id_from_sign_pk_hex, KeyPair};
use polygone_core::task::TaskResult;
use serde::{Deserialize, Serialize};

use crate::engine::{Transport, TransportError};
use crate::model::{NodeCapabilities, Scope, TaskEnvelope as Envelope, WIRE_VERSION};
use crate::now_unix;

/// Taille maximale d'une ligne de protocole : 2 Mio.
pub const MAX_WIRE_BYTES: usize = 2 * 1024 * 1024;
/// Délai d'E/S par défaut.
pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Connexions simultanées par défaut.
pub const MAX_CONNECTIONS: usize = 32;

/// Requête de fil.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WireRequest {
    Capabilities,
    Run { envelope: Envelope },
}

/// Réponse de fil.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WireResponse {
    Capabilities { capabilities: NodeCapabilities },
    Ok { result: TaskResult },
    Error { code: String, message: String },
}

/// Vérifie qu'un nœud peut recevoir cette enveloppe.
///
/// C'est la seule autorisation du prototype : périmètre, consentement,
/// ressources, priorité utilisateur. Aucun droit général n'est accordé à
/// l'appelant.
pub fn authorize(
    node: &NodeCapabilities,
    envelope: &Envelope,
    now: u64,
) -> Result<(), TransportError> {
    envelope.validate().map_err(TransportError::Malformed)?;
    if envelope.v != WIRE_VERSION {
        return Err(TransportError::Refused {
            code: "protocol".to_string(),
            message: "version de protocole inconnue".to_string(),
        });
    }
    match envelope.scope {
        Scope::Local => {
            if node.scope != Scope::Local || node.local_fabric != envelope.fabric {
                return Err(TransportError::Refused {
                    code: "scope".to_string(),
                    message: "nœud hors du périmètre local de la tâche".to_string(),
                });
            }
        }
        Scope::Global => {
            if !envelope.global_consent {
                return Err(TransportError::Refused {
                    code: "consent".to_string(),
                    message: "consentement global absent côté demandeur".to_string(),
                });
            }
            if node.scope != Scope::Global {
                return Err(TransportError::Refused {
                    code: "scope".to_string(),
                    message: "nœud hors du périmètre global de la tâche".to_string(),
                });
            }
            if !node.global_consent {
                return Err(TransportError::Refused {
                    code: "consent".to_string(),
                    message: "le nœud n'a pas consenti au périmètre global".to_string(),
                });
            }
        }
    }
    envelope
        .task
        .validate(now)
        .map_err(|error| TransportError::Refused {
            code: "task".to_string(),
            message: error.to_string(),
        })?;
    node.can_accept_remote(&envelope.task)
        .map_err(|message| TransportError::Refused {
            code: "resources".to_string(),
            message,
        })?;
    Ok(())
}

fn signed_result(
    node: &NodeCapabilities,
    keypair: &KeyPair,
    envelope: &Envelope,
    now: u64,
) -> Result<TaskResult, TransportError> {
    let output = envelope
        .task
        .kind
        .execute(
            &envelope.task.input,
            envelope.task.constraints.permissions.max_output_bytes,
        )
        .map_err(|error| TransportError::Execution(error.to_string()))?;
    let result = TaskResult::signed(
        envelope.task.id.clone(),
        output,
        node.node_id.clone(),
        now,
        keypair,
    )
    .map_err(|error| TransportError::Execution(error.to_string()))?;
    result
        .verify()
        .map_err(|error| TransportError::Execution(format!("résultat auto-invalide : {error}")))?;
    Ok(result)
}

/// Un nœud serveur en mémoire.
pub struct ServerNode {
    pub capabilities: NodeCapabilities,
    keypair: KeyPair,
    pub max_connections: usize,
}

impl ServerNode {
    /// Construit un serveur et vérifie que le `NodeId` annoncé est bien dérivé
    /// de la clé de signature. Un label libre ne suffit pas.
    pub fn new(capabilities: NodeCapabilities, keypair: KeyPair) -> Result<Self, String> {
        capabilities.validate()?;
        let derived = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
            .ok_or_else(|| "clé ML-DSA-65 invalide".to_string())?;
        if derived != capabilities.node_id {
            return Err(format!(
                "node_id {} ne dérive pas de la clé {}",
                capabilities.node_id, derived
            ));
        }
        Ok(Self {
            capabilities,
            keypair,
            max_connections: MAX_CONNECTIONS,
        })
    }

    /// Exécute une enveloppe après autorisation et rend un résultat signé.
    pub fn execute(&self, envelope: &Envelope) -> Result<TaskResult, TransportError> {
        authorize(&self.capabilities, envelope, now_unix())?;
        signed_result(&self.capabilities, &self.keypair, envelope, now_unix())
    }
}

/// Serveur TCP : accepte des requêtes NDJSON, une par connexion.
pub fn serve(listener: TcpListener, node: Arc<ServerNode>) -> io::Result<()> {
    let live = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("fabric: accept impossible : {error}");
                continue;
            }
        };
        if live.load(Ordering::SeqCst) >= node.max_connections {
            eprintln!("fabric: plafond de connexions atteint — refus");
            let _ = stream.shutdown(std::net::Shutdown::Both);
            continue;
        }
        live.fetch_add(1, Ordering::SeqCst);
        let node = Arc::clone(&node);
        let live_guard = LiveGuard(Arc::clone(&live));
        std::thread::spawn(move || {
            let _guard = live_guard;
            if let Err(error) = handle_connection(stream, &node) {
                eprintln!("fabric: connexion terminée : {error}");
            }
        });
    }
    Ok(())
}

struct LiveGuard(Arc<AtomicUsize>);

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn handle_connection(mut stream: TcpStream, node: &ServerNode) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;

    let Some(line) = read_line(&mut stream, MAX_WIRE_BYTES)? else {
        return Ok(());
    };
    let request: WireRequest = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(error) => {
            return send_response(
                &mut stream,
                &WireResponse::Error {
                    code: "malformed".to_string(),
                    message: format!("requête illisible : {error}"),
                },
            );
        }
    };

    match request {
        WireRequest::Capabilities => send_response(
            &mut stream,
            &WireResponse::Capabilities {
                capabilities: node.capabilities.clone(),
            },
        ),
        WireRequest::Run { envelope } => match node.execute(&envelope) {
            Ok(result) => send_response(&mut stream, &WireResponse::Ok { result }),
            Err(TransportError::Refused { code, message }) => {
                send_response(&mut stream, &WireResponse::Error { code, message })
            }
            Err(error) => send_response(
                &mut stream,
                &WireResponse::Error {
                    code: "execution".to_string(),
                    message: error.to_string(),
                },
            ),
        },
    }
}

fn send_response(stream: &mut TcpStream, response: &WireResponse) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(response).map_err(io::Error::other)?;
    bytes.push(b'\n');
    stream.write_all(&bytes)
}

/// Lit une ligne bornée. Rend `None` si le flux se ferme, si la ligne dépasse
/// le plafond ou si le délai expire — jamais une ligne tronquée présentée
/// comme complète.
pub fn read_line(stream: &mut impl Read, cap: usize) -> io::Result<Option<String>> {
    let mut buffer = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                if byte[0] == b'\n' {
                    return Ok(Some(
                        String::from_utf8_lossy(&buffer)
                            .trim_end_matches('\r')
                            .to_string(),
                    ));
                }
                buffer.push(byte[0]);
                if buffer.len() > cap {
                    return Ok(None);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

/// Transport TCP vers un ensemble de pairs.
pub struct TcpTransport {
    peers: Vec<SocketAddr>,
    timeout: Duration,
}

impl TcpTransport {
    pub fn new(peers: Vec<SocketAddr>) -> Self {
        Self {
            peers,
            timeout: IO_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn peers(&self) -> &[SocketAddr] {
        &self.peers
    }

    fn request(
        &self,
        peer: SocketAddr,
        request: &WireRequest,
    ) -> Result<WireResponse, TransportError> {
        let mut stream = TcpStream::connect_timeout(&peer, self.timeout)
            .map_err(|error| TransportError::Unavailable(format!("{peer} : {error}")))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| TransportError::Io(error.to_string()))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let mut bytes = serde_json::to_vec(request)
            .map_err(|error| TransportError::Malformed(error.to_string()))?;
        bytes.push(b'\n');
        stream
            .write_all(&bytes)
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let line = read_line(&mut stream, MAX_WIRE_BYTES)
            .map_err(|error| TransportError::Io(error.to_string()))?
            .ok_or_else(|| TransportError::Malformed("réponse absente".to_string()))?;
        serde_json::from_str(&line)
            .map_err(|error| TransportError::Malformed(format!("réponse illisible : {error}")))
    }

    fn capabilities_of(&self, peer: SocketAddr) -> Option<NodeCapabilities> {
        match self.request(peer, &WireRequest::Capabilities) {
            Ok(WireResponse::Capabilities { capabilities }) => Some(capabilities),
            _ => None,
        }
    }
}

impl Transport for TcpTransport {
    fn list_nodes(&self) -> Vec<NodeCapabilities> {
        self.peers
            .iter()
            .filter_map(|peer| self.capabilities_of(*peer))
            .collect()
    }

    fn dispatch(&self, node: &str, envelope: &Envelope) -> Result<TaskResult, TransportError> {
        for peer in &self.peers {
            let Some(capabilities) = self.capabilities_of(*peer) else {
                continue;
            };
            if capabilities.node_id != node {
                continue;
            }
            return match self.request(
                *peer,
                &WireRequest::Run {
                    envelope: envelope.clone(),
                },
            )? {
                WireResponse::Ok { result } => Ok(result),
                WireResponse::Error { code, message } => {
                    Err(TransportError::Refused { code, message })
                }
                WireResponse::Capabilities { .. } => Err(TransportError::Malformed(
                    "le pair a répondu avec des capacités à une demande d'exécution".to_string(),
                )),
            };
        }
        Err(TransportError::Unavailable(format!(
            "nœud {node} introuvable sur les pairs configurés"
        )))
    }
}

/// Nœud en mémoire, pour les tests et la démonstration.
pub struct MemoryNode {
    pub capabilities: NodeCapabilities,
    keypair: KeyPair,
    pub fail: bool,
    pub tamper: bool,
}

impl MemoryNode {
    pub fn new(capabilities: NodeCapabilities, keypair: KeyPair) -> Result<Self, String> {
        capabilities.validate()?;
        let derived = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex())
            .ok_or_else(|| "clé ML-DSA-65 invalide".to_string())?;
        if derived != capabilities.node_id {
            return Err("node_id incohérent avec la clé".to_string());
        }
        Ok(Self {
            capabilities,
            keypair,
            fail: false,
            tamper: false,
        })
    }

    pub fn failing(mut self) -> Self {
        self.fail = true;
        self
    }

    pub fn tampering(mut self) -> Self {
        self.tamper = true;
        self
    }

    pub fn keypair() -> KeyPair {
        generate_keypair().expect("génération ML-DSA-65")
    }
}

/// Transport en mémoire : exécute et signe réellement, sans socket.
pub struct MemoryTransport {
    pub nodes: Vec<MemoryNode>,
}

impl MemoryTransport {
    pub fn new(nodes: Vec<MemoryNode>) -> Self {
        Self { nodes }
    }
}

impl Transport for MemoryTransport {
    fn list_nodes(&self) -> Vec<NodeCapabilities> {
        self.nodes
            .iter()
            .map(|node| node.capabilities.clone())
            .collect()
    }

    fn dispatch(&self, node_id: &str, envelope: &Envelope) -> Result<TaskResult, TransportError> {
        let node = self
            .nodes
            .iter()
            .find(|node| node.capabilities.node_id == node_id)
            .ok_or_else(|| TransportError::Unavailable(format!("nœud {node_id} inconnu")))?;
        if node.fail {
            return Err(TransportError::Unavailable(format!(
                "nœud {node_id} simulé en panne"
            )));
        }
        authorize(&node.capabilities, envelope, now_unix())?;
        let mut result = signed_result(&node.capabilities, &node.keypair, envelope, now_unix())?;
        if node.tamper {
            if let Some(first) = result.output.first_mut() {
                *first ^= 0x01;
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::FabricEngine;
    use crate::planner::PlacementPolicy;
    use polygone_core::task::{
        TaskConstraints, TaskId, TaskKind, TaskPermissions, TaskRequirements, TaskSpec,
    };
    use std::sync::atomic::AtomicU64;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn task(kind: TaskKind, input: &[u8], cores: u32) -> TaskSpec {
        let id = format!(
            "task_wire_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let now = now_unix();
        TaskSpec {
            id: TaskId::parse(id).unwrap(),
            owner: "test".to_string(),
            kind,
            input: input.to_vec(),
            requirements: TaskRequirements {
                cpu_cores: cores,
                ram_bytes: 1024,
                storage_bytes: 0,
                gpu_vram_bytes: 0,
            },
            constraints: TaskConstraints {
                deadline_unix: now + 300,
                max_attempts: 2,
                permissions: TaskPermissions::default(),
            },
        }
    }

    fn caps(id: &str, cores: u32, load: u16) -> NodeCapabilities {
        NodeCapabilities {
            node_id: id.to_string(),
            scope: Scope::Local,
            global_consent: false,
            local_fabric: "test".to_string(),
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

    fn envelope(task: TaskSpec, scope: Scope, global_consent: bool) -> Envelope {
        Envelope {
            v: WIRE_VERSION,
            scope,
            global_consent,
            fabric: "test".to_string(),
            requester: "requester".to_string(),
            task,
        }
    }

    fn local_engine(id: &str, load: u16) -> FabricEngine {
        let keypair = MemoryNode::keypair();
        let derived = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        let mut local = caps(&derived, 1, load);
        local.rtt_ms = 0;
        let _ = id;
        FabricEngine::new(local, keypair, PlacementPolicy::default()).unwrap()
    }

    #[test]
    fn memory_transport_executes_and_signs() {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        let transport =
            MemoryTransport::new(vec![MemoryNode::new(caps(&id, 8, 0), keypair).unwrap()]);
        let task = task(TaskKind::Blake3, b"hello", 2);
        let envelope = envelope(task.clone(), Scope::Local, false);
        let result = transport.dispatch(&id, &envelope).expect("exécution");
        assert_eq!(result.task_id, task.id);
        assert_eq!(result.output, blake3::hash(b"hello").to_hex().as_bytes());
        result.verify().expect("signature vérifiée");
    }

    #[test]
    fn global_task_is_refused_by_a_local_node() {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        let node = ServerNode::new(caps(&id, 8, 0), keypair).unwrap();
        let envelope = envelope(task(TaskKind::Echo, b"x", 1), Scope::Global, true);
        let error = node.execute(&envelope).expect_err("périmètre refusé");
        assert!(
            matches!(error, TransportError::Refused { ref code, .. } if code == "scope"),
            "{error:?}"
        );
    }

    #[test]
    fn user_activity_refuses_a_remote_task() {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        let mut capabilities = caps(&id, 8, 0);
        capabilities.user_active = true;
        let node = ServerNode::new(capabilities, keypair).unwrap();
        let error = node
            .execute(&envelope(
                task(TaskKind::Echo, b"x", 1),
                Scope::Local,
                false,
            ))
            .expect_err("utilisateur actif");
        assert!(
            matches!(error, TransportError::Refused { ref code, .. } if code == "resources"),
            "{error:?}"
        );
    }

    #[test]
    fn real_tcp_round_trip_reaches_a_remote_node() {
        let keypair = MemoryNode::keypair();
        let id = node_id_from_sign_pk_hex(&keypair.verifier.public_key().to_hex()).unwrap();
        let capabilities = caps(&id, 8, 0);
        let node = Arc::new(ServerNode::new(capabilities, keypair).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind éphémère");
        let address = listener.local_addr().expect("adresse");
        std::thread::spawn(move || {
            let _ = serve(listener, node);
        });

        let transport = TcpTransport::new(vec![address]).with_timeout(Duration::from_secs(3));
        let found = transport.list_nodes();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].node_id, id);

        let mut engine = local_engine("local", 980);
        let envelope = envelope(task(TaskKind::Blake3, b"tcp-proof", 2), Scope::Local, false);
        let outcome = engine.execute(&envelope, now_unix(), Some(&transport));
        assert!(outcome.verified, "{outcome:?}");
        let result = outcome.result.expect("résultat");
        assert_eq!(result.producer, id);
        assert_eq!(
            result.output,
            blake3::hash(b"tcp-proof").to_hex().as_bytes()
        );
    }
}
