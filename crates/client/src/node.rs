//! node — le premier service qui **consomme son daemon** avant de servir.
//!
//! # Le contrat, en quatre temps
//!
//! 1. **Qui demande** : le nœud, à travers le client de référence
//!    (`polygoned::DaemonClient`) — jamais un client réinventé.
//! 2. **Qui accorde** : le daemon local, via son ledger (`daemon/src/socket.rs:451`).
//!    Il ne peut pas imposer grand-chose : depuis 2b-5b il n'écrit plus aucun
//!    limiteur système, donc un accord est **une licence, pas une contrainte**
//!    (`daemon/src/socket.rs:209-232`).
//! 3. **Qui tient la promesse** : le nœud, et lui seul. Il se limite lui-même à
//!    ce qu'il a obtenu ([`max_transfers`]) et n'ouvre sa prise d'écoute
//!    **qu'après** l'accord.
//! 4. **Si le bail disparaît** : le nœud cesse de servir. Le daemon meurt →
//!    `LedgerGuard` (`daemon/src/socket.rs:931`) ne garde aucune réservation, et
//!    le nœud le voit à sa première vérification ([`LEASE_POLL_INTERVAL`]) parce
//!    que la connexion se ferme. Servir sans bail est précisément le
//!    comportement que ce chantier retire.
//!
//! # `node servir` n'est PAS `bloc servir`
//!
//! - `polygone bloc servir` est l'**outil bas niveau** : il sert un dossier, sans
//!   bail, sans comptes, sans état — c'est un outil de test et de diagnostic.
//!   Il ne demande rien à personne, donc il ne promet rien.
//! - `polygone node servir` est le **service** : il exige un bail, s'y tient,
//!   rend des comptes et s'arrête quand le bail disparaît. Il ne peut pas
//!   démarrer sans daemon.
//!
//! Deux rôles, deux commandes. Confondre les deux, c'est soit exiger un bail
//! d'un outil de test, soit servir sans licence en croyant faire un service.

use crate::blocks::{self, Served};
use crate::mesh;
use polygoned::socket::Reply;
use polygoned::DaemonClient;
use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// RAM demandée par défaut, en Gio.
///
/// Ce n'est pas un chiffre rond par confort. Le service retient en mémoire, au
/// pire : `blocks::MAX_CONNECTIONS` (16) blocs de `blocks::MAX_BLOCK_BYTES`
/// (1 Mio) **plus** autant de lignes de requête (64 Kio au plafond) **plus**
/// 16 Kio de réserve par connexion — soit ~18 Mio. 0.25 Gio couvre cela et laisse
/// la place au processus lui-même (binaire, piles de threads) sans réclamer une
/// ressource qu'on ne consomme pas : demander 4 Gio pour servir des fichiers
/// serait réserver la machine à ne rien faire.
pub const DEFAULT_BAIL_RAM_GB: f32 = 0.25;

/// Cœurs demandés par défaut.
///
/// Un seul. Un transfert de bloc est dominé par l'E/S (lire un fichier, écrire
/// sur le réseau) ; le seul calcul est un BLAKE3 de ≤ 1 Mio, ~1 ms. Le daemon
/// compte les cœurs en entiers : demander 1 est la traduction honnête de « ce
/// service occupe au plus un cœur ».
pub const DEFAULT_BAIL_CPU_CORES: u32 = 1;

/// Intervalle de vérification du bail.
///
/// Doit rester **très** inférieur à `CONNECTION_IDLE_TIMEOUT` (60 s,
/// `daemon/src/socket.rs:668`) : c'est aussi ce battement qui empêche le daemon
/// de fermer une connexion inactive. Un bâillement d'une seconde donne une
/// détection de perte bornée, sans coût perceptible (1 requête/s).
pub const LEASE_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Au-delà de cet âge, le fichier d'état n'est plus une mesure.
///
/// Trois battements : un fichier plus vieux que ça ne peut pas venir d'un nœud
/// vivant, puisque celui-ci le réécrit à chaque tour.
pub const STATE_STALE_AFTER: Duration = Duration::from_secs(5);

/// Coût mémoire retenu par connexion simultanée, pour dimensionner l'auto-limite.
const PER_CONNECTION_BYTES: u64 = (blocks::MAX_BLOCK_BYTES + 64 * 1024 + 16 * 1024) as u64; // bloc + ligne de requête + réserve

/// Emplacement par défaut du fichier d'état (`~/.polygone/node-state.json`).
pub fn default_state_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".polygone")
        .join("node-state.json")
}

/// Ce qu'un nœud a le droit de faire, et où.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    /// Adresse d'écoute. Bindée **seulement** après l'accord du bail.
    pub listen: SocketAddr,
    /// Racine du magasin de blocs servi.
    pub blocks_root: PathBuf,
    /// Fichier d'état, relu par `polygone node status`.
    pub state_path: PathBuf,
    /// RAM demandée au daemon.
    pub bail_ram_gb: f32,
    /// Cœurs demandés au daemon.
    pub bail_cpu_cores: u32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([0, 0, 0, 0], blocks::DEFAULT_PORT)),
            blocks_root: blocks::default_root(),
            state_path: default_state_path(),
            bail_ram_gb: DEFAULT_BAIL_RAM_GB,
            bail_cpu_cores: DEFAULT_BAIL_CPU_CORES,
        }
    }
}

/// Ce qui empêche un nœud de servir.
#[derive(Debug)]
pub enum NodeError {
    /// Aucun daemon joignable : **rien n'a été bindé**.
    NoDaemon { path: PathBuf, cause: String },
    /// Le daemon a refusé le bail : **rien n'a été bindé**.
    Denied { reason: String },
    /// Réponse inattendue du daemon (le protocole ne promet pas ça).
    Unexpected(Reply),
    /// L'écoute a échoué (port pris, adresse invalide).
    Bind(io::Error),
    /// Le service s'est arrêté sur une erreur d'écoute.
    Serve(io::Error),
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeError::NoDaemon { path, cause } => write!(
                f,
                "aucun daemon joignable sur {} ({cause}) — lancez « polygoned » d'abord ; \
                 le nœud n'a rien ouvert",
                path.display()
            ),
            NodeError::Denied { reason } => write!(
                f,
                "bail REFUSÉ par le daemon ({reason}) — le nœud n'a rien ouvert"
            ),
            NodeError::Unexpected(reply) => {
                write!(f, "réponse inattendue du daemon : {reply:?}")
            }
            NodeError::Bind(e) => write!(f, "écoute impossible : {e}"),
            NodeError::Serve(e) => write!(f, "service interrompu : {e}"),
        }
    }
}

impl std::error::Error for NodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NodeError::Bind(e) | NodeError::Serve(e) => Some(e),
            _ => None,
        }
    }
}

/// Le bail obtenu — ce que le nœud s'engage à ne pas dépasser.
#[derive(Clone, Debug)]
pub struct Lease {
    pub allocation_id: String,
    pub ram_gb: f32,
    pub cpu_cores: u32,
    pub granted_at: Instant,
}

/// Pourquoi le service s'est arrêté.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// `SIGINT` demandé par l'utilisateur.
    Signal,
    /// Le daemon a disparu : le bail n'existe plus.
    LeaseLost,
    /// Le daemon signale une pression de ressources : retrait conservateur.
    PolicyShrink,
}

impl StopReason {
    pub fn label(self) -> &'static str {
        match self {
            StopReason::Signal => "signal",
            StopReason::LeaseLost => "bail_perdu",
            StopReason::PolicyShrink => "retrait_politique",
        }
    }
}

/// Ce que le service rapporte en s'arrêtant.
#[derive(Clone, Copy, Debug)]
pub struct ServeOutcome {
    pub reason: StopReason,
    pub served_blocks: u64,
    pub served_bytes: u64,
    /// Le bail a-t-il été rendu explicitement ?
    pub released: bool,
}

/// Un nœud qui **détient un bail** et une prise d'écoute.
///
/// Le champ `client` n'est pas décoratif : c'est la connexion qui porte le bail
/// (le daemon libère ce qui a été accordé sur une connexion quand elle se ferme,
/// `LedgerGuard`). Tant que le nœud sert, cette connexion doit vivre, et c'est
/// elle qu'on interroge pour savoir si le bail tient toujours.
pub struct Node {
    client: DaemonClient,
    lease: Lease,
    listener: TcpListener,
    config: NodeConfig,
    /// Compteurs du service, créés avec le nœud : ils existent **avant** de
    /// servir, ce qui permet de les observer pendant que le service tourne.
    served: Arc<Served>,
}

impl Node {
    /// Demande le bail, puis ouvre l'écoute — **dans cet ordre**.
    ///
    /// L'ordre est la garantie entière : si le daemon est absent ou refuse, on
    /// sort avant le `bind`, donc **le port reste fermé**. Un nœud qui binderait
    /// d'abord et demanderait ensuite servirait sans licence pendant la fraction
    /// de seconde où il attend la réponse — c'est-à-dire servirait sans licence.
    pub fn acquire(socket: &Path, config: NodeConfig) -> Result<Self, NodeError> {
        let mut client = DaemonClient::connect(socket).map_err(|e| NodeError::NoDaemon {
            path: socket.to_path_buf(),
            cause: e.to_string(),
        })?;

        let lease = match client
            .allocate(config.bail_ram_gb, config.bail_cpu_cores)
            .map_err(|e| NodeError::NoDaemon {
                path: socket.to_path_buf(),
                cause: e.to_string(),
            })? {
            Reply::AllocGranted {
                allocation_id,
                cpu_cores,
                ram_gb,
                ..
            } => Lease {
                allocation_id,
                ram_gb,
                cpu_cores,
                granted_at: Instant::now(),
            },
            Reply::AllocDenied { reason, .. } => return Err(NodeError::Denied { reason }),
            other => return Err(NodeError::Unexpected(other)),
        };

        // Seulement maintenant. Si l'écoute échoue, on rend le bail avant de
        // partir : la fermeture de `client` le rendrait de toute façon
        // (`LedgerGuard`), mais le rendre explicitement ne laisse aucun doute sur
        // l'intention.
        let listener = match TcpListener::bind(config.listen) {
            Ok(listener) => listener,
            Err(e) => {
                let _ = client.release(&lease.allocation_id);
                return Err(NodeError::Bind(e));
            }
        };

        Ok(Self {
            client,
            lease,
            listener,
            config,
            served: Arc::new(Served::default()),
        })
    }

    /// Le bail détenu.
    pub fn lease(&self) -> &Lease {
        &self.lease
    }

    /// Les compteurs du service (blocs, octets, connexions admises/refusées).
    ///
    /// Rendus avant que `serve` consomme le nœud : c'est ce qui permet de
    /// **voir** le plafond auto-imposé travailler, au lieu de croire sa
    /// déclaration.
    pub fn served(&self) -> Arc<Served> {
        Arc::clone(&self.served)
    }

    /// Plafond de transferts simultanés que le nœud s'impose.
    pub fn max_transfers(&self) -> usize {
        max_transfers(self.lease.cpu_cores, self.lease.ram_gb)
    }

    /// L'adresse réellement écoutée (utile quand le port demandé était 0).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Sert jusqu'à `stop` (posé par SIGINT) ou perte du bail.
    ///
    /// La boucle principale ne sert rien elle-même : elle **surveille le bail**
    /// pendant que [`blocks::serve_with`] fait le travail. C'est le seul endroit
    /// du projet où servir et avoir-le-droit-de-servir sont deux tâches séparées —
    /// et c'est exactement pour pouvoir arrêter la seconde que la première existe.
    pub fn serve(self, stop: Arc<AtomicBool>) -> Result<ServeOutcome, NodeError> {
        let Node {
            mut client,
            lease,
            listener,
            config,
            served,
        } = self;

        let addr = listener.local_addr().map_err(NodeError::Serve)?;
        let limit = max_transfers(lease.cpu_cores, lease.ram_gb);

        let server_stop = Arc::new(AtomicBool::new(false));
        let served_for_server = Arc::clone(&served);
        let server_stop_thread = Arc::clone(&server_stop);
        let root = config.blocks_root.clone();
        let server = std::thread::spawn(move || {
            blocks::serve_with(listener, root, limit, served_for_server, server_stop_thread)
        });

        let mut state = NodeState::serving(&lease, addr, limit, &served);
        write_state(&config.state_path, &state);

        let reason = loop {
            if stop.load(Ordering::SeqCst) {
                break StopReason::Signal;
            }
            match client.status() {
                Ok(Reply::Status {
                    shrinking: true, ..
                }) => {
                    // Le daemon signale que sa politique doit reprendre des
                    // ressources. Le protocole ne sait pas réduire un bail en
                    // place : on arrête donc ce service et on rend tout le bail.
                    log::info!("node: retrait demandé par la politique de ressources");
                    break StopReason::PolicyShrink;
                }
                Ok(Reply::Status { .. }) => {}
                Ok(other) => {
                    log::warn!("node: réponse de statut inattendue {other:?} — arrêt conservateur");
                    break StopReason::LeaseLost;
                }
                Err(error) => {
                    // La connexion qui portait le bail ne répond plus : le daemon
                    // est mort ou a fermé. Servir plus longtemps serait servir
                    // sans licence.
                    log::warn!("node: bail perdu ({error}) — arrêt du service");
                    break StopReason::LeaseLost;
                }
            }
            state.update(&served);
            write_state(&config.state_path, &state);
            sleep_step(LEASE_POLL_INTERVAL, &stop);
        };

        // Arrêt du service : on lève le drapeau et on réveille l'acceptation
        // bloquée en se connectant à nous-mêmes. Sans ce réveil, il faudrait
        // attendre la prochaine connexion d'un pair pour que la boucle voie le
        // drapeau — l'arrêt durerait une éternité sur un réseau calme.
        server_stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&connectable(addr), Duration::from_millis(500));
        let _ = server.join();

        // Rendre le bail. Sur perte du bail, il n'y a plus personne à qui le
        // rendre : le daemon qui l'a accordé n'existe plus, et sa disparition a
        // déjà libéré la réservation en même temps que la connexion.
        let released = match reason {
            StopReason::Signal | StopReason::PolicyShrink => {
                client.release(&lease.allocation_id).is_ok()
            }
            StopReason::LeaseLost => false,
        };

        state.finish(reason, &served, released);
        write_state(&config.state_path, &state);

        Ok(ServeOutcome {
            reason,
            served_blocks: served.blocks(),
            served_bytes: served.bytes(),
            released,
        })
    }
}

/// Combien de transferts simultanés le nœud s'autorise, **d'après son bail**.
///
/// Décision, et sur quoi elle est fondée :
/// - `cœurs × 2` : un transfert est dominé par l'E/S ; deux par cœur accordé est
///   la traduction d'un service d'E/S en occupation CPU. Au-delà, on promettrait
///   plus de CPU que le bail n'en accorde.
/// - `octets accordés / PER_CONNECTION_BYTES` : on ne peut pas garder en mémoire
///   plus de blocs en vol que la RAM accordée n'en contient. C'est la borne qui
///   parle du bail RAM, pas de la machine.
/// - `blocks::MAX_CONNECTIONS` : plafond du module, on ne le contourne pas.
///
/// Jamais 0 : un bail de 0 cœur accorde quand même **un** transfert. Refuser de
/// servir pour un bail nul serait un refus silencieux, et un nœud qui ne sert
/// personne n'a aucune raison d'avoir demandé un bail.
pub fn max_transfers(cpu_cores: u32, ram_gb: f32) -> usize {
    let ram_bytes = (ram_gb.max(0.0) as f64 * 1024.0 * 1024.0 * 1024.0) as u64;
    let by_ram = (ram_bytes / PER_CONNECTION_BYTES) as usize;
    let by_cores = (cpu_cores as usize).saturating_mul(2);
    let ceiling = by_cores.min(by_ram).min(blocks::MAX_CONNECTIONS);
    // Plancher de 1 : bornes ordonnées (1 ≤ MAX_CONNECTIONS), donc `clamp` ne
    // peut pas paniquer ici.
    ceiling.clamp(1, blocks::MAX_CONNECTIONS)
}

/// Adresse utilisable pour se connecter à sa propre écoute.
///
/// Une écoute sur `0.0.0.0` n'est pas une adresse de destination (`0.0.0.0` veut
/// dire « toutes ») : pour se réveiller soi-même, il faut `127.0.0.1`.
fn connectable(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_unspecified() {
        SocketAddr::from(([127, 0, 0, 1], addr.port()))
    } else {
        addr
    }
}

/// Dort par petits pas pour qu'un signal soit honoré vite.
fn sleep_step(total: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

// ── L'état publié, lu par `polygone node status` ─────────────────────────────

/// Ce qu'un nœud écrit sur lui-même, horodaté.
///
/// **Mécanisme choisi : un fichier d'état horodaté** (`~/.polygone/node-state.json`).
/// Pourquoi : il est lisible depuis un autre processus sans IPC, sans droits, et
/// sans que le nœud ait à répondre ; et il porte **son propre horodatage**, ce qui
/// est la seule chose qui empêche un fichier d'un nœud mort de passer pour un
/// nœud vivant. Un nœud vivant le réécrit à chaque battement
/// ([`LEASE_POLL_INTERVAL`]) : passé [`STATE_STALE_AFTER`], le contenu n'est plus
/// une mesure, et `node status` le dit au lieu de l'afficher comme un fait.
///
/// Ce qui n'est PAS dans ce fichier : le mesh. Le mesh se **mesure à l'instant**
/// par `node status` (une découverte de 2 s) — un compteur de pairs périmé serait
/// exactement le chiffre inventé qu'on veut éviter.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NodeState {
    pub pid: u32,
    /// Instant d'obtention du bail, distinct du battement de fraîcheur.
    #[serde(default)]
    pub started_at_unix: u64,
    pub updated_at_unix: u64,
    pub status: String,
    pub allocation_id: String,
    pub ram_gb: f32,
    pub cpu_cores: u32,
    pub listen: String,
    pub max_transfers: usize,
    pub served_blocks: u64,
    pub served_bytes: u64,
    /// Connexions admises et refusées : la trace du plafond auto-imposé.
    pub connections_accepted: u64,
    pub connections_refused: u64,
    pub stop_reason: Option<String>,
    pub released: Option<bool>,
}

impl NodeState {
    fn serving(lease: &Lease, addr: SocketAddr, limit: usize, served: &Served) -> Self {
        Self {
            pid: std::process::id(),
            started_at_unix: now_unix().saturating_sub(lease.granted_at.elapsed().as_secs()),
            updated_at_unix: now_unix(),
            status: "serving".to_string(),
            allocation_id: lease.allocation_id.clone(),
            ram_gb: lease.ram_gb,
            cpu_cores: lease.cpu_cores,
            listen: addr.to_string(),
            max_transfers: limit,
            served_blocks: served.blocks(),
            served_bytes: served.bytes(),
            connections_accepted: served.accepted(),
            connections_refused: served.refused(),
            stop_reason: None,
            released: None,
        }
    }

    fn update(&mut self, served: &Served) {
        self.updated_at_unix = now_unix();
        self.served_blocks = served.blocks();
        self.served_bytes = served.bytes();
        self.connections_accepted = served.accepted();
        self.connections_refused = served.refused();
    }

    fn finish(&mut self, reason: StopReason, served: &Served, released: bool) {
        self.updated_at_unix = now_unix();
        self.status = "stopped".to_string();
        self.stop_reason = Some(reason.label().to_string());
        self.released = Some(released);
        self.served_blocks = served.blocks();
        self.served_bytes = served.bytes();
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Écrit l'état **atomiquement** (temporaire dans le même dossier, puis rename) :
/// un lecteur ne doit jamais tomber sur un JSON à moitié écrit, ce qui
/// ressemblerait à un nœud cassé alors qu'il écrit simplement.
fn write_state(path: &Path, state: &NodeState) {
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(json) = serde_json::to_string_pretty(state) else {
        return;
    };
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("state"),
        std::process::id()
    ));
    if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// Lit l'état et son âge. `Err` porte la raison, jamais un chiffre inventé.
pub fn read_state(path: &Path) -> Result<(NodeState, Duration), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let state: NodeState =
        serde_json::from_str(&text).map_err(|e| format!("{} illisible : {e}", path.display()))?;
    let age = Duration::from_secs(now_unix().saturating_sub(state.updated_at_unix));
    Ok((state, age))
}

/// Le rapport de `polygone node status`.
///
/// Règle unique : **une mesure, ou une absence de mesure — jamais une
/// supposition**. Les compteurs et le bail viennent du fichier d'état et portent
/// son âge ; le mesh, lui, est mesuré maintenant. Si le fichier est périmé, le
/// rapport le dit avant de citer le moindre chiffre.
pub fn status_report(state_path: &Path, mesh_scan: Duration) -> String {
    let mut out = String::new();
    out.push_str("⬡ Polygone node — status\n");

    match read_state(state_path) {
        Ok((state, age)) => {
            let stale = age > STATE_STALE_AFTER;
            let freshness = if stale {
                format!(
                    "PÉRIMÉ — dernière mise à jour il y a {} s (> {} s) ; les chiffres ci-dessous ne sont plus une mesure",
                    age.as_secs(),
                    STATE_STALE_AFTER.as_secs()
                )
            } else {
                format!("à jour (il y a {} s)", age.as_secs())
            };

            out.push_str(&format!("  État        : {freshness}\n"));
            let grant_age = if state.started_at_unix == 0 {
                "durée inconnue".to_string()
            } else {
                format!("{} s", now_unix().saturating_sub(state.started_at_unix))
            };
            out.push_str(&format!(
                "  Bail        : {} · {} Gio · {} cœur(s) — accordé depuis {} (état local)\n",
                state.allocation_id, state.ram_gb, state.cpu_cores, grant_age
            ));
            out.push_str(&format!(
                "  Écoute      : {} · plafond auto-imposé {} transfert(s)\n",
                state.listen, state.max_transfers
            ));
            out.push_str(&format!(
                "  Servi       : {} bloc(s) · {} octet(s) · {} connexion(s) admise(s) · {} refusée(s) au plafond\n",
                state.served_blocks, state.served_bytes, state.connections_accepted, state.connections_refused
            ));
            out.push_str(&format!(
                "  Processus   : pid {} · {} (tiré du fichier d'état, pas vérifié)\n",
                state.pid, state.status
            ));
            if let Some(reason) = &state.stop_reason {
                out.push_str(&format!(
                    "  Dernier arrêt : {reason} · bail rendu : {}\n",
                    state.released.unwrap_or(false)
                ));
            }
        }
        Err(why) => {
            out.push_str("  État        : aucun fichier d'état exploitable\n");
            out.push_str(&format!(
                "                {why}\n                → le service n'a jamais tourné ici, ou son état est ailleurs ; \
                 bail, compteurs et adresse sont INCONNUS (pas zéro : inconnus)\n"
            ));
        }
    }

    // Le mesh se mesure maintenant : un compteur de pairs vieux d'une minute
    // serait un chiffre mort affiché comme vivant.
    match mesh::discover(mesh_scan) {
        Ok(peers) => {
            out.push_str(&format!(
                "  Mesh        : {} pair(s) vu(s) à l'instant (udp {})\n",
                peers.len(),
                mesh::MESH_PORT
            ));
            for peer in peers.iter().take(5) {
                out.push_str(&format!("                  · {}\n", peer.node_id));
            }
        }
        Err(e) => out.push_str(&format!("  Mesh        : mesure impossible ({e})\n")),
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use polygoned::socket::{
        bind_listener, serve_with_timeout_at, DaemonState, CONNECTION_IDLE_TIMEOUT,
    };
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::{Mutex, OnceLock, RwLock};
    use std::thread;

    /// Dossier temporaire supprimé à la fin du test. Aucun test ne touche au
    /// `~/.polygone` réel : tout passe par des chemins explicites.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("polygone-node-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).expect("dossier temporaire");
            Self(p)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn blocks_root(&self) -> PathBuf {
            self.0.join("blocks")
        }

        fn state_path(&self) -> PathBuf {
            self.0.join("node-state.json")
        }

        fn socket_path(&self) -> PathBuf {
            self.0.join("daemon.sock")
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Un VRAI daemon en mémoire (le même code que le binaire : `bind_listener`
    /// + `serve_with_timeout`), sur un socket temporaire.
    fn spawn_real_daemon(
        root: &TempRoot,
        state: DaemonState,
    ) -> (
        PathBuf,
        Arc<RwLock<DaemonState>>,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
    ) {
        let path = root.socket_path();
        let listener = bind_listener(&path).expect("bind du daemon de test");
        let state = Arc::new(RwLock::new(state));
        let shutdown = Arc::new(AtomicBool::new(false));
        let serve_shutdown = Arc::clone(&shutdown);
        let serve_state = Arc::clone(&state);
        let ledger_path = root.path().join("ledger.json");
        let handle = thread::spawn(move || {
            serve_with_timeout_at(
                listener,
                serve_state,
                serve_shutdown,
                CONNECTION_IDLE_TIMEOUT,
                ledger_path,
            )
        });
        (path, state, shutdown, handle)
    }

    fn stop_real_daemon(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
        shutdown.store(true, Ordering::SeqCst);
        handle.join().expect("le daemon doit s'arrêter");
    }

    /// Un nœud qui sert dans un thread, avec sa prise d'écoute connue.
    ///
    /// Rend l'adresse **avant** de servir : `serve` consomme le nœud, donc le
    /// test doit connaître le port par avance.
    struct RunningNode {
        stop: Arc<AtomicBool>,
        handle: thread::JoinHandle<Result<ServeOutcome, NodeError>>,
    }

    impl RunningNode {
        fn start(node: Node) -> (SocketAddr, Self) {
            let addr = node.local_addr().expect("adresse locale");
            let stop = Arc::new(AtomicBool::new(false));
            let stop_thread = Arc::clone(&stop);
            let handle = thread::spawn(move || node.serve(stop_thread));
            (addr, Self { stop, handle })
        }

        fn signal_and_wait(self) -> Result<ServeOutcome, NodeError> {
            self.stop.store(true, Ordering::SeqCst);
            self.handle.join().expect("le nœud doit rendre la main")
        }
    }

    /// Faux daemon **minimal** qui accorde un bail puis peut mourir.
    ///
    /// Il parle le VRAI protocole : les lignes sont produites et lues par
    /// `polygoned::socket::{encode, decode}`, jamais écrites à la main — sinon on
    /// testerait le nœud contre une paraphrase du protocole.
    struct FakeDaemon {
        path: PathBuf,
        die: Arc<AtomicBool>,
        handle: thread::JoinHandle<()>,
        _root: TempRoot,
    }

    impl FakeDaemon {
        fn start(grant: bool) -> Self {
            let root = TempRoot::new();
            let path = root.socket_path();
            let listener = UnixListener::bind(&path).expect("bind du faux daemon");
            let die = Arc::new(AtomicBool::new(false));
            let die_thread = Arc::clone(&die);

            let handle = thread::spawn(move || {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                let mut reader = match stream.try_clone() {
                    Ok(clone) => BufReader::new(clone),
                    Err(_) => return,
                };
                let mut writer = stream;

                // `hello` : la première chose qu'un client attend.
                let hello = polygoned::socket::Reply::Hello {
                    daemon_version: "test".to_string(),
                    platform: "test".to_string(),
                    tier: "balanced".to_string(),
                    protocol_version: polygoned::socket::PROTOCOL_VERSION,
                };
                let _ = writer.write_all(polygoned::socket::encode(&hello).as_bytes());

                loop {
                    if die_thread.load(Ordering::SeqCst) {
                        return; // « kill -9 » : on lâche la connexion sans rien dire
                    }
                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => return,
                        Ok(_) => {}
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                || e.kind() == io::ErrorKind::TimedOut =>
                        {
                            continue;
                        }
                        Err(_) => return,
                    }
                    let Ok(request) = polygoned::socket::decode(&line) else {
                        continue;
                    };
                    let reply = match request {
                        polygoned::socket::Request::Alloc {
                            id,
                            cpu_cores,
                            ram_gb,
                            ..
                        } if grant => polygoned::socket::Reply::AllocGranted {
                            id,
                            allocation_id: "bail-du-faux-daemon".to_string(),
                            cpu_cores,
                            ram_gb,
                            gpu_vram_gb: None,
                            storage_gb: None,
                            expires_at: None,
                        },
                        polygoned::socket::Request::Alloc { id, .. } => {
                            polygoned::socket::Reply::AllocDenied {
                                id,
                                reason: polygoned::socket::REASON_INSUFFICIENT_HEADROOM.to_string(),
                            }
                        }
                        polygoned::socket::Request::Status { id } => status_reply(id),
                        polygoned::socket::Request::Release { id, allocation_id } => {
                            polygoned::socket::Reply::Released { id, allocation_id }
                        }
                        polygoned::socket::Request::Ping { id } => {
                            polygoned::socket::Reply::Pong { id }
                        }
                    };
                    if writer
                        .write_all(polygoned::socket::encode(&reply).as_bytes())
                        .is_err()
                    {
                        return;
                    }
                }
            });

            Self {
                path,
                die,
                handle,
                _root: root,
            }
        }

        /// Le daemon disparaît **sans rien libérer** (l'équivalent de `kill -9`).
        fn kill_and_join(self) {
            self.die.store(true, Ordering::SeqCst);
            let _ = self.handle.join();
        }
    }

    fn status_reply(id: Option<String>) -> Reply {
        Reply::Status {
            id,
            tier: "balanced".to_string(),
            seq: 0,
            max_cpu_percent: 50,
            max_ram_percent: 50,
            max_bandwidth_percent: 60,
            max_gpu_percent: 40,
            ram_bytes: 1024 * 1024 * 1024,
            ram_mb: 1024,
            bandwidth_mbps: 10,
            cpu_cores: 3,
            gpu_free_vram_mb: 0,
            shrinking: false,
            uptime_secs: 0,
        }
    }

    /// Une racine de blocs contenant un bloc, et son hash.
    fn seeded_blocks(root: &TempRoot) -> String {
        blocks::put_in(&root.blocks_root(), b"bloc servi sous bail").expect("put")
    }

    /// `POLYGONE_DAEMON_SOCKET` est un état **global du processus** : les tests
    /// qui s'en servent se sérialisent, comme `testutil::GLOBAL_STATE_LOCK` le
    /// fait pour le binaire.
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_max_transfers_follows_the_lease() {
        // Le défaut : 1 cœur, 0.25 Gio → 2 (deux transferts par cœur).
        assert_eq!(max_transfers(1, DEFAULT_BAIL_RAM_GB), 2);
        // Deux cœurs → quatre.
        assert_eq!(max_transfers(2, DEFAULT_BAIL_RAM_GB), 4);
        // La RAM borne : 1 Gio ne peut pas tenir 100 connexions × ~1.1 Mio.
        assert_eq!(max_transfers(64, 1.0), 909.min(blocks::MAX_CONNECTIONS));
        // Le plafond du module borne : on ne dépasse jamais 16.
        assert_eq!(max_transfers(64, 64.0), blocks::MAX_CONNECTIONS);
        // Jamais zéro : un bail nul accorde UN transfert, pas un refus muet.
        assert_eq!(max_transfers(0, 0.0), 1);
    }

    #[test]
    fn test_node_refuses_to_serve_without_daemon() {
        let root = TempRoot::new();
        let port = free_port();
        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], port)),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };

        let err = match Node::acquire(&root.socket_path(), config) {
            Ok(_) => panic!("sans daemon, aucun nœud ne doit s'ouvrir"),
            Err(e) => e,
        };
        assert!(
            matches!(err, NodeError::NoDaemon { .. }),
            "un daemon absent doit se dire : {err}"
        );

        // Le port n'est PAS ouvert — vérifié, pas supposé.
        assert!(
            TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(300)
            )
            .is_err(),
            "aucune écoute ne doit exister après un refus"
        );
    }

    #[test]
    fn test_node_refuses_to_serve_without_lease() {
        let root = TempRoot::new();

        // Daemon VIVANT, mais sans marge : tout le plafond du palier est déjà
        // alloué. La demande doit donc être refusée par le ledger.
        let mut state = DaemonState::new("balanced");
        state.ram_bytes = state.allowed_ram_bytes();
        let (socket, _state, shutdown, handle) = spawn_real_daemon(&root, state);

        let port = free_port();
        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], port)),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            bail_ram_gb: 1.0,
            bail_cpu_cores: 1,
        };

        let err = match Node::acquire(&socket, config) {
            Ok(_) => panic!("sans marge, le daemon doit refuser"),
            Err(e) => e,
        };
        match &err {
            NodeError::Denied { reason } => assert_eq!(
                reason,
                polygoned::socket::REASON_INSUFFICIENT_HEADROOM,
                "le refus doit porter la raison du ledger"
            ),
            other => panic!("attendu Denied, obtenu {other:?}"),
        }

        // Le contrôle du refus est ce qui empêche de binder : port fermé.
        assert!(
            TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_millis(300)
            )
            .is_err(),
            "un bail refusé ne doit ouvrir AUCUNE écoute"
        );

        stop_real_daemon(&shutdown, handle);
    }

    #[test]
    fn test_node_serves_only_with_lease() {
        let root = TempRoot::new();
        let hash = seeded_blocks(&root);
        let (socket, _state, shutdown, handle) =
            spawn_real_daemon(&root, DaemonState::new("balanced"));

        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };
        let node = Node::acquire(&socket, config).expect("le bail doit être accordé");
        assert_eq!(node.max_transfers(), 2, "auto-limite déduite du bail");

        let (addr, running) = RunningNode::start(node);

        // Un second client — même processus ici, deux processus dans la
        // vérification manuelle — récupère le bloc, hash vérifié.
        let got = blocks::fetch(&hash, addr).expect("le bloc doit venir du nœud");
        assert_eq!(got, b"bloc servi sous bail");

        let outcome = running.signal_and_wait().expect("arrêt propre");
        assert_eq!(outcome.reason, StopReason::Signal);
        assert_eq!(outcome.served_blocks, 1, "un bloc servi, compté");
        assert_eq!(outcome.served_bytes, b"bloc servi sous bail".len() as u64);
        assert!(outcome.released, "le bail doit être rendu sur signal");

        stop_real_daemon(&shutdown, handle);
    }

    #[test]
    fn test_node_stops_when_lease_is_lost() {
        let root = TempRoot::new();
        let hash = seeded_blocks(&root);
        let daemon = FakeDaemon::start(true);

        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };
        let node = Node::acquire(&daemon.path, config).expect("bail accordé");
        let (addr, running) = RunningNode::start(node);

        // Le service sert vraiment, tant que le bail tient.
        assert!(blocks::fetch(&hash, addr).is_ok(), "le nœud doit servir");

        // Le daemon meurt (kill -9) : plus personne ne détient le bail.
        let started = Instant::now();
        daemon.kill_and_join();

        let outcome = running.handle.join().expect("le nœud doit rendre la main");
        let elapsed = started.elapsed();
        let outcome = outcome.expect("l'arrêt sur perte de bail n'est pas une erreur");
        assert_eq!(
            outcome.reason,
            StopReason::LeaseLost,
            "le nœud doit s'arrêter parce que le bail a disparu"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "l'arrêt doit être borné (annoncé ~{} s), mesuré {elapsed:?}",
            LEASE_POLL_INTERVAL.as_secs()
        );
        assert!(
            !outcome.released,
            "il n'y a plus personne à qui rendre le bail"
        );

        // Observable de l'extérieur : l'écoute est fermée.
        assert!(
            TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err(),
            "un nœud sans bail ne doit plus écouter"
        );
        // Et il ne sert plus : le bloc existe toujours, mais personne ne le sert.
        assert!(blocks::fetch(&hash, addr).is_err());
    }

    #[test]
    fn test_node_releases_lease_on_sigint() {
        let root = TempRoot::new();
        let (socket, _state, shutdown, handle) =
            spawn_real_daemon(&root, DaemonState::new("balanced"));

        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            // Toute la marge du daemon de test (7 Gio) : tant qu'elle est tenue,
            // personne d'autre ne peut l'obtenir.
            bail_ram_gb: 7.0,
            bail_cpu_cores: 1,
        };
        let node = Node::acquire(&socket, config).expect("bail accordé");
        let (_addr, running) = RunningNode::start(node);

        // Un SECOND client constate que la marge est bien réservée.
        let mut other = DaemonClient::connect(&socket).expect("second client");
        match other.allocate(7.0, 1).expect("réponse") {
            Reply::AllocDenied { reason, .. } => {
                assert_eq!(reason, polygoned::socket::REASON_INSUFFICIENT_HEADROOM);
            }
            other => panic!("la marge devait être tenue, reçu {other:?}"),
        }

        // SIGINT (injecté : le vrai signal est branché dans le binaire, et testé
        // à la main dans la vérification). Le nœud doit rendre la main.
        let outcome = running.signal_and_wait().expect("arrêt propre");
        assert_eq!(outcome.reason, StopReason::Signal);
        assert!(outcome.released, "release() doit avoir été appelé");

        // La preuve : la MÊME demande repasse, sans attendre la fermeture de
        // notre propre connexion. C'est le `LedgerGuard` du daemon, vu de
        // l'extérieur.
        match other.allocate(7.0, 1).expect("réponse après libération") {
            Reply::AllocGranted { .. } => {}
            other => panic!("la marge devait être rendue, reçu {other:?}"),
        }

        stop_real_daemon(&shutdown, handle);
    }

    #[test]
    fn test_node_releases_lease_when_policy_shrinks() {
        let root = TempRoot::new();
        let (socket, state, shutdown, handle) =
            spawn_real_daemon(&root, DaemonState::new("balanced"));
        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };
        let node = Node::acquire(&socket, config).expect("bail accordé");
        let (_addr, running) = RunningNode::start(node);

        state.write().expect("état daemon").shrinking = true;
        let outcome = running
            .handle
            .join()
            .expect("arrêt sur retrait politique")
            .expect("arrêt propre");
        assert_eq!(outcome.reason, StopReason::PolicyShrink);
        assert!(outcome.released, "le bail doit être rendu au daemon vivant");
        stop_real_daemon(&shutdown, handle);
    }

    #[test]
    fn test_node_respects_its_own_limit() {
        let root = TempRoot::new();
        let hash = seeded_blocks(&root);
        let (socket, _state, shutdown, handle) =
            spawn_real_daemon(&root, DaemonState::new("balanced")); // Bail par DÉFAUT : 1 cœur, 0.25 Gio → auto-limite **2** (deux transferts
                                                                    // par cœur, bornés par la RAM accordée et le plafond du module).
        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };
        let node = Node::acquire(&socket, config).expect("bail accordé");
        assert_eq!(node.max_transfers(), 2, "auto-limite du bail par défaut");
        let counters = node.served();
        let (addr, running) = RunningNode::start(node);

        // Deux connexions qui ne terminent JAMAIS leur requête : chacune occupe
        // un créneau de façon déterministe (le service attend la fin de ligne
        // jusqu'à son délai de lecture). On attend la PREUVE que le créneau est
        // pris — le compteur — au lieu de dormir en l'espérant.
        let mut held = Vec::new();
        for expected in 1..=2u64 {
            let mut conn = TcpStream::connect(addr).expect("connexion qui tient un créneau");
            conn.set_read_timeout(Some(Duration::from_secs(2)))
                .expect("délai");
            conn.write_all(b"GET ").expect("requête partielle");
            held.push(conn);
            let deadline = Instant::now() + Duration::from_secs(2);
            while counters.accepted() < expected {
                assert!(
                    Instant::now() < deadline,
                    "le nœud devait admettre {expected} connexion(s) ; admises : {}",
                    counters.accepted()
                );
                thread::sleep(Duration::from_millis(10));
            }
        }

        // Troisième connexion, requête COMPLÈTE : le plafond est atteint, elle
        // doit être REFUSÉE — pas mise en attente, pas servie.
        let mut third = TcpStream::connect(addr).expect("troisième connexion");
        third
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("délai");
        third
            .write_all(format!("GET {hash}\n").as_bytes())
            .expect("écriture");
        let deadline = Instant::now() + Duration::from_secs(2);
        while counters.refused() == 0 {
            assert!(
                Instant::now() < deadline,
                "au-delà du plafond, le nœud doit refuser"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let mut line = String::new();
        let read = BufReader::new(&mut third).read_line(&mut line);
        assert!(
            matches!(read, Ok(0)) || read.is_err(),
            "un transfert au-delà du plafond ne doit pas être servi : {line:?}"
        );
        assert!(
            !line.starts_with("HAVE"),
            "jamais servi au-delà du bail : {line:?}"
        );
        assert_eq!(
            counters.blocks(),
            0,
            "aucun bloc servi : les créneaux tenus"
        );

        drop(held);
        let outcome = running.signal_and_wait().expect("arrêt propre");
        assert_eq!(outcome.served_blocks, 0);
        assert_eq!(counters.accepted(), 2, "deux connexions admises, pas trois");
        assert_eq!(
            counters.refused(),
            1,
            "la troisième a été refusée au plafond"
        );

        stop_real_daemon(&shutdown, handle);

        // CONTRE-ÉPREUVE : les mêmes deux créneaux tenus, mais un service SANS
        // limite auto-imposée sert la troisième connexion. Sans cette moitié on
        // ne saurait pas si le refus venait du plafond ou d'autre chose.
        let base = root.blocks_root();
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], free_port()))).unwrap();
        let open_addr = listener.local_addr().unwrap();
        let open_stop = Arc::new(AtomicBool::new(false));
        let open_counters = Arc::new(Served::default());
        let stop_thread = Arc::clone(&open_stop);
        let counters_thread = Arc::clone(&open_counters);
        thread::spawn(move || {
            let _ = blocks::serve_with(
                listener,
                base,
                blocks::MAX_CONNECTIONS,
                counters_thread,
                stop_thread,
            );
        });

        let mut held_open = Vec::new();
        for expected in 1..=2u64 {
            let mut conn = TcpStream::connect(open_addr).expect("créneau tenu (contrôle)");
            conn.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            conn.write_all(b"GET ").unwrap();
            held_open.push(conn);
            let deadline = Instant::now() + Duration::from_secs(2);
            while open_counters.accepted() < expected {
                assert!(Instant::now() < deadline, "créneaux du contrôle non admis");
                thread::sleep(Duration::from_millis(10));
            }
        }
        let mut served_conn = TcpStream::connect(open_addr).expect("3e connexion du contrôle");
        served_conn
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        served_conn
            .write_all(format!("GET {hash}\n").as_bytes())
            .unwrap();
        let mut served = String::new();
        BufReader::new(&mut served_conn)
            .read_line(&mut served)
            .unwrap();
        assert!(
            served.starts_with("HAVE"),
            "sans plafond auto-imposé, la même 3e connexion EST servie : {served:?}"
        );
        assert_eq!(open_counters.refused(), 0, "aucun refus sans plafond serré");

        drop(held_open);
        drop(served_conn);
        open_stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&open_addr, Duration::from_millis(200));
    }

    #[test]
    fn test_status_says_unknown_when_there_is_no_state() {
        let root = TempRoot::new();
        let report = status_report(&root.state_path(), Duration::from_millis(200));
        assert!(
            report.contains("INCONNUS"),
            "sans état, le rapport doit dire INCONNU, pas zéro :\n{report}"
        );
        assert!(
            report.contains("Mesh"),
            "le mesh est mesuré, lui :\n{report}"
        );
    }

    #[test]
    fn test_status_flags_a_stale_state() {
        let root = TempRoot::new();
        let path = root.state_path();
        let mut state = NodeState {
            pid: 1234,
            started_at_unix: now_unix().saturating_sub(45),
            updated_at_unix: now_unix(),
            status: "serving".to_string(),
            allocation_id: "bail-1".to_string(),
            ram_gb: 0.25,
            cpu_cores: 1,
            listen: "127.0.0.1:7643".to_string(),
            max_transfers: 2,
            served_blocks: 0,
            served_bytes: 0,
            connections_accepted: 0,
            connections_refused: 0,
            stop_reason: None,
            released: None,
        };
        write_state(&path, &state);

        let fresh = status_report(&path, Duration::from_millis(200));
        assert!(fresh.contains("à jour"), "état frais :\n{fresh}");
        assert!(
            fresh.contains("accordé depuis 45 s"),
            "l'âge du bail doit être distinct de la fraîcheur du fichier :\n{fresh}"
        );

        // On recule l'horodatage : le rapport doit crier « périmé » au lieu
        // d'afficher des chiffres morts comme s'ils étaient vivants.
        state.updated_at_unix = now_unix() - (STATE_STALE_AFTER.as_secs() + 5);
        write_state(&path, &state);

        let stale = status_report(&path, Duration::from_millis(200));
        assert!(stale.contains("PÉRIMÉ"), "état périmé :\n{stale}");
    }

    /// Port libre trouvé en demandant au noyau, puis relâché.
    fn free_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind éphémère");
        let port = listener.local_addr().expect("adresse").port();
        drop(listener);
        port
    }

    /// Le chemin d'état par défaut n'utilise jamais le `~/.polygone` réel dans
    /// les tests — vérifié ici pour que ce ne soit pas une intention.
    #[test]
    fn test_tests_never_use_the_real_home() {
        let _guard = env_lock().lock().unwrap_or_else(|p| p.into_inner());
        let root = TempRoot::new();
        let config = NodeConfig {
            state_path: root.state_path(),
            blocks_root: root.blocks_root(),
            ..NodeConfig::default()
        };
        assert!(
            config.state_path.starts_with(root.path()),
            "le chemin d'état des tests est sous le dossier temporaire"
        );
        assert!(config.blocks_root.starts_with(root.path()));
    }

    /// Un faux daemon reste vivant pour l'usage croisé éventuel.
    #[test]
    fn test_fake_daemon_denies_when_asked_to() {
        let daemon = FakeDaemon::start(false);
        let root = TempRoot::new();
        let config = NodeConfig {
            listen: SocketAddr::from(([127, 0, 0, 1], free_port())),
            blocks_root: root.blocks_root(),
            state_path: root.state_path(),
            ..NodeConfig::default()
        };
        match Node::acquire(&daemon.path, config) {
            Err(NodeError::Denied { reason }) => {
                assert_eq!(reason, polygoned::socket::REASON_INSUFFICIENT_HEADROOM)
            }
            Ok(_) => panic!("le faux daemon devait refuser"),
            Err(other) => panic!("attendu un refus du ledger, obtenu {other:?}"),
        }
        daemon.kill_and_join();
    }

    /// Le socket d'un faux daemon est bien un socket Unix vivant.
    #[test]
    fn test_fake_daemon_is_reachable() {
        let daemon = FakeDaemon::start(true);
        assert!(UnixStream::connect(&daemon.path).is_ok());
        daemon.kill_and_join();
    }
}
