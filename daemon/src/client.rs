//! Client de référence du protocole v1 — la moitié manquante du socket.
//!
//! Jusqu'à 2b-6, le daemon ne savait que **servir** : `encode(&Reply)` et
//! `decode(&str) -> Request` n'avaient pas de symétriques, donc tout client
//! devait réinventer son JSON. `crates/petals/src/daemon.rs` l'a fait, et mal :
//! chemin `/tmp/polygoned.sock`, pas de champ `v`, noms de messages différents
//! (`docs/PLAN_CONSOMMATEUR.md` §3). Ce module existe pour qu'il n'y ait plus
//! qu'UNE façon de parler au daemon, et pour qu'elle soit testée contre le VRAI
//! serveur.
//!
//! Bloquant, `std` + `serde_json` uniquement : aucun runtime asynchrone, comme
//! le reste du crate. Aucun JSON écrit à la main — tout passe par
//! `socket::encode_request` / `socket::decode_reply`, ce qui interdit au client
//! de dériver du serveur sans qu'un test le voie.

use crate::socket::{
    decode_reply, encode_request, Reply, Request, CONNECTION_IDLE_TIMEOUT, PROTOCOL_VERSION,
};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Nom porté par le champ `client` des demandes d'allocation.
///
/// Le serveur ne s'en sert que pour dériver un `allocation_id` stable : le nom
/// dit QUI a demandé, il n'autorise rien et ne réserve rien de plus.
const CLIENT_NAME: &str = "daemon-client";

/// Ce qui peut mal tourner côté client.
///
/// Trois natures volontairement séparées : un problème de transport, un désaccord
/// de protocole (version, `id`), et un refus **décidé** par le serveur. Les
/// confondre rendrait impossible le diagnostic d'un daemon muet.
#[derive(Debug)]
pub enum ClientError {
    /// Le transport a échoué (connexion, délai d'attente, E/S).
    Io(std::io::Error),
    /// Le fil ne dit pas ce que le protocole promet : version inconnue, `id` qui
    /// ne correspond pas, réponse illisible.
    Protocol(String),
    /// Le serveur a refusé et l'a dit (`Reply::Error { code, message }`).
    Server { code: String, message: String },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Io(e) => write!(f, "E/S : {e}"),
            ClientError::Protocol(m) => write!(f, "protocole : {m}"),
            ClientError::Server { code, message } => {
                write!(f, "refus du serveur [{code}] : {message}")
            }
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ClientError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ClientError {
    fn from(err: std::io::Error) -> Self {
        ClientError::Io(err)
    }
}

/// Client bloquant : une connexion, une requête, une réponse.
pub struct DaemonClient {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    /// Compteur d'identifiants, strictement croissant. C'est lui qui permet de
    /// dire « cette réponse n'est pas la mienne » au lieu de la croire.
    next_id: u64,
}

impl DaemonClient {
    /// Se connecte avec les délais d'un **consommateur longue durée**.
    ///
    /// C'est le passe-partout : les délais valent `CONNECTION_IDLE_TIMEOUT`, la
    /// valeur du serveur, parce qu'une connexion qui travaille doit pouvoir
    /// rester ouverte. Une **sonde de diagnostic** ne doit PAS passer par ici —
    /// elle hériterait de ces mêmes délais et pendrait une minute sur un daemon
    /// muet. Pour une sonde, voir `connect_with_timeout`.
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        Self::connect_with_timeout(path, CONNECTION_IDLE_TIMEOUT)
    }

    /// Se connecte en imposant SON délai, en lecture ET en écriture, puis lit le
    /// `hello` et **refuse** une version qu'il ne connaît pas.
    ///
    /// Le délai est un paramètre, pas une constante, parce que le bon délai
    /// dépend de l'USAGE et non du protocole : une minute pour un consommateur
    /// qui travaille, une seconde pour une sonde qui doit répondre vite. Les deux
    /// parlent exactement le même protocole — seule la PATIENCE change.
    ///
    /// La version est vérifiée ici, une fois, plutôt qu'à chaque réponse : une
    /// connexion qui a accepté un `hello` d'une autre version ne doit pas servir
    /// ensuite, et il n'y a rien à rattraper.
    pub fn connect_with_timeout(path: &Path, timeout: Duration) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path)?;

        // Le délai s'applique aux DEUX sens : sans lui, un daemon muet ferait
        // bloquer le client pour toujours, et un client qui n'écoute pas ses
        // réponses ferait bloquer le serveur.
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let reader = BufReader::new(stream.try_clone()?);

        let mut client = Self {
            stream,
            reader,
            next_id: 0,
        };

        match client.read_reply()? {
            Reply::Hello {
                protocol_version, ..
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(ClientError::Protocol(format!(
                        "version de protocole inconnue : le daemon annonce {protocol_version}, \
                         ce client parle {PROTOCOL_VERSION}"
                    )));
                }
            }
            other => {
                return Err(ClientError::Protocol(format!(
                    "première réponse attendue `hello`, reçue {other:?}"
                )));
            }
        }

        Ok(client)
    }

    /// Envoie une requête, lit **une** réponse, vérifie que l'`id` correspond.
    ///
    /// L'`id` de la requête passée est ignoré et remplacé par un identifiant
    /// unique et croissant : c'est le seul moyen de garantir la corrélation, et
    /// cela évite qu'un appelant réutilise un `id` par distraction.
    pub fn request(&mut self, request: Request) -> Result<Reply, ClientError> {
        self.next_id += 1;
        let id = format!("c{}", self.next_id);
        let line = encode_request(&with_id(request, id.clone()));

        self.stream.write_all(line.as_bytes())?;
        self.stream.flush()?;

        let reply = self.read_reply()?;

        // Un `Reply::Error` SANS `id` est accepté comme refus du serveur : c'est
        // exactement ce qu'il produit quand il n'a pas pu corréler la requête
        // (plafond de connexions atteint, ligne indécodable). En revanche un
        // autre type de réponse portant un AUTRE `id` est une désynchronisation
        // du flux : ce n'est pas une réponse, on ne l'interprète pas.
        if let Reply::Error {
            id: None,
            code,
            message,
        } = &reply
        {
            return Err(ClientError::Server {
                code: code.clone(),
                message: message.clone(),
            });
        }

        if reply_id(&reply) != Some(id.as_str()) {
            return Err(ClientError::Protocol(format!(
                "id de réponse inattendu : attendu {id:?}, reçu {:?}",
                reply_id(&reply)
            )));
        }

        // Le refus décidé par le serveur n'est pas une panne de transport : il a
        // un code et un message, et l'appelant doit pouvoir les lire tels quels.
        if let Reply::Error { code, message, .. } = &reply {
            return Err(ClientError::Server {
                code: code.clone(),
                message: message.clone(),
            });
        }

        Ok(reply)
    }

    /// État courant de la décision d'allocation.
    pub fn status(&mut self) -> Result<Reply, ClientError> {
        self.request(Request::Status { id: None })
    }

    /// Demande une allocation.
    ///
    /// Un accord est une **licence, pas une contrainte** (voir `Reply::AllocGranted`) :
    /// le daemon enregistre la réservation pour ne pas sur-accorder, mais c'est à
    /// l'appelant de s'y tenir.
    pub fn allocate(&mut self, ram_gb: f32, cpu_cores: u32) -> Result<Reply, ClientError> {
        self.request(Request::Alloc {
            id: None,
            client: Some(CLIENT_NAME.to_string()),
            cpu_cores,
            ram_gb,
            gpu_vram_gb: None,
            storage_gb: None,
        })
    }

    /// Rend explicitement une allocation.
    ///
    /// La réponse est `released` **même si l'identifiant était inconnu** : la
    /// libération est idempotente, et un client qui a déjà libéré ne doit pas
    /// échouer pour autant.
    pub fn release(&mut self, allocation_id: &str) -> Result<Reply, ClientError> {
        self.request(Request::Release {
            id: None,
            allocation_id: allocation_id.to_string(),
        })
    }

    /// Lit une ligne et la décode en réponse.
    fn read_reply(&mut self) -> Result<Reply, ClientError> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line)?;

        if n == 0 {
            // Fin de flux : le serveur a fermé sans rien dire. C'est un problème
            // de transport, pas un refus — d'où `Io`, malgré l'absence d'errno.
            return Err(ClientError::Io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "le serveur a fermé la connexion sans répondre",
            )));
        }

        decode_reply(&line).map_err(|e| {
            ClientError::Protocol(format!("réponse illisible ({e}) : {}", line.trim()))
        })
    }
}

/// Remplace l'`id` d'une requête par celui du client.
fn with_id(request: Request, id: String) -> Request {
    match request {
        Request::Status { .. } => Request::Status { id: Some(id) },
        Request::Ping { .. } => Request::Ping { id: Some(id) },
        Request::Alloc {
            client,
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            storage_gb,
            ..
        } => Request::Alloc {
            id: Some(id),
            client,
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            storage_gb,
        },
        Request::Release { allocation_id, .. } => Request::Release {
            id: Some(id),
            allocation_id,
        },
    }
}

/// L'`id` porté par une réponse, s'il y en a un (`hello` n'en a pas).
fn reply_id(reply: &Reply) -> Option<&str> {
    match reply {
        Reply::Hello { .. } => None,
        Reply::Status { id, .. }
        | Reply::AllocGranted { id, .. }
        | Reply::AllocDenied { id, .. }
        | Reply::Released { id, .. }
        | Reply::Pong { id, .. }
        | Reply::Error { id, .. } => id.as_deref(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::{
        bind_listener, serve_with_timeout, DaemonState, REASON_INSUFFICIENT_HEADROOM,
    };
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, RwLock};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Un VRAI serveur sur un socket temporaire. Le `TempDir` rendu doit rester
    /// vivant : c'est lui qui possède le dossier. État de référence : 7 Gio de
    /// marge (8 autorisés − 1 déjà alloué) et 3 cœurs.
    fn start_server() -> (
        PathBuf,
        Arc<AtomicBool>,
        thread::JoinHandle<()>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("daemon.sock");
        let listener = bind_listener(&path).expect("bind sur un socket temporaire");
        // Le ledger est isolé par test, comme le socket : sans ça, un test hérite
        // des réservations laissées par un test précédent (`insufficient_headroom`).
        std::env::set_var(
            crate::socket::ENV_LEDGER_PATH,
            dir.path().join("ledger.json"),
        );
        let state = Arc::new(RwLock::new(DaemonState::new("balanced")));
        let shutdown = Arc::new(AtomicBool::new(false));

        let serve_shutdown = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            serve_with_timeout(listener, state, serve_shutdown, CONNECTION_IDLE_TIMEOUT)
        });

        (path, shutdown, handle, dir)
    }

    fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<()>) {
        shutdown.store(true, Ordering::SeqCst);
        handle.join().expect("le serveur doit s'arrêter proprement");
    }

    /// Ligne `hello` fabriquée, avec la version demandée.
    fn hello_line(protocol_version: u8) -> String {
        format!(
            "{{\"v\":1,\"type\":\"hello\",\"daemon_version\":\"2.0.0\",\"platform\":\"test\",\
             \"tier\":\"balanced\",\"protocol_version\":{protocol_version}}}\n"
        )
    }

    /// Faux serveur : écrit `hello`, attend UNE requête, puis écrit `reply`.
    ///
    /// C'est le seul moyen de déclencher les gardes du CLIENT — le vrai serveur
    /// ne se trompe ni d'`id` ni de version. Tout est tolérant aux erreurs d'E/S :
    /// un client qui refuse le `hello` ferme la connexion, et le faux serveur ne
    /// doit pas paniquer pour autant (sinon le test échouerait pour la mauvaise
    /// raison).
    fn start_fake_server(
        hello: String,
        reply: String,
    ) -> (PathBuf, thread::JoinHandle<()>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("fake.sock");
        let listener = UnixListener::bind(&path).expect("bind du faux serveur");

        let handle = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = stream.write_all(hello.as_bytes());
            let _ = stream.flush();

            // On attend la requête : l'ordre est ainsi garanti sans `sleep`.
            let mut reader = match stream.try_clone() {
                Ok(clone) => BufReader::new(clone),
                Err(_) => return,
            };
            let mut line = String::new();
            let _ = reader.read_line(&mut line);

            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.flush();
        });

        (path, handle, dir)
    }

    /// Faux serveur MUET : il accepte la connexion et n'écrit **jamais**.
    ///
    /// C'est la panne qui compte ici : un daemon qui garde son socket ouvert mais
    /// ne répond plus. Sans délai court, un client y resterait jusqu'à
    /// `CONNECTION_IDLE_TIMEOUT` — une minute.
    fn start_silent_server() -> (PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("dossier temporaire");
        let path = dir.path().join("muet.sock");
        let listener = UnixListener::bind(&path).expect("bind du serveur muet");

        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                // Le flux reste ouvert ET silencieux ; le thread le garde le temps
                // que le test constate le silence.
                thread::sleep(Duration::from_secs(5));
                drop(stream);
            }
        });

        (path, dir)
    }

    #[test]
    fn test_client_reads_hello_and_checks_version() {
        let (path, shutdown, handle, _dir) = start_server();

        // `connect` consomme le `hello` : la première requête doit donc recevoir
        // la réponse À ELLE, sinon la salutation serait lue comme un `status_response`.
        let mut client = DaemonClient::connect(&path).expect("connexion au vrai daemon");
        let reply = client.status().expect("le hello a bien été consommé");
        assert!(matches!(reply, Reply::Status { .. }), "reçu {reply:?}");

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_client_status_roundtrip() {
        let (path, shutdown, handle, _dir) = start_server();
        let mut client = DaemonClient::connect(&path).expect("connexion");

        match client.status().expect("status") {
            Reply::Status {
                tier,
                seq,
                ram_mb,
                cpu_cores,
                ..
            } => {
                assert_eq!(tier, "balanced");
                assert_eq!(seq, 0, "un DaemonState neuf part de zéro");
                assert_eq!(ram_mb, 1024, "1 Gio déjà alloué dans l'état de test");
                assert_eq!(cpu_cores, 3);
            }
            other => panic!("attendu Status, obtenu {other:?}"),
        }

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_client_alloc_then_release() {
        let (path, shutdown, handle, _dir) = start_server();
        let mut client = DaemonClient::connect(&path).expect("connexion");

        let allocation_id = match client.allocate(5.0, 1).expect("allocation") {
            Reply::AllocGranted {
                allocation_id,
                cpu_cores,
                ram_gb,
                ..
            } => {
                assert_eq!(cpu_cores, 1);
                assert_eq!(ram_gb, 5.0);
                assert!(!allocation_id.is_empty(), "un accord porte un identifiant");
                allocation_id
            }
            other => panic!("attendu AllocGranted, obtenu {other:?}"),
        };

        match client.release(&allocation_id).expect("libération") {
            Reply::Released {
                allocation_id: released,
                ..
            } => assert_eq!(
                released, allocation_id,
                "la libération porte le même identifiant"
            ),
            other => panic!("attendu Released, obtenu {other:?}"),
        }

        // La marge est rendue tout de suite : la MÊME demande repasse.
        assert!(
            matches!(
                client.allocate(5.0, 1).expect("seconde allocation"),
                Reply::AllocGranted { .. }
            ),
            "release doit rendre la marge sans attendre la fermeture"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_client_sees_server_error_as_server_error() {
        let (path, shutdown, handle, _dir) = start_server();
        let mut client = DaemonClient::connect(&path).expect("connexion");

        // Le seul `Reply::Error` qu'un client v1 bien élevé puisse déclencher est
        // le refus de débit : on envoie donc une rafale jusqu'au refus. Aucun
        // `sleep`, donc la rafale tient très largement dans la fenêtre d'une
        // seconde du serveur (51 allers-retours coûtent quelques millisecondes).
        let mut refusal = None;
        for _ in 0..300 {
            if let Err(err) = client.status() {
                refusal = Some(err);
                break;
            }
        }

        match refusal.expect("le serveur doit finir par refuser la rafale") {
            ClientError::Server { code, message } => {
                assert_eq!(
                    code, "rate_limited",
                    "le code vient du serveur, pas du transport"
                );
                assert!(!message.is_empty(), "un refus doit s'expliquer");
            }
            other => panic!("un refus doit être un ClientError::Server, obtenu {other:?}"),
        }

        stop_server(&shutdown, handle);
    }

    /// LE test qui ferme la boucle : deux clients réels, un seul daemon, et une
    /// marge qui circule d'un client disparu vers un client vivant.
    #[test]
    fn test_two_clients_share_one_daemon() {
        let (path, shutdown, handle, _dir) = start_server();

        let mut a = DaemonClient::connect(&path).expect("connexion de A");
        let mut b = DaemonClient::connect(&path).expect("connexion de B");

        // A prend TOUTE la marge de l'état de test (7 Gio), 1 cœur.
        assert!(
            matches!(
                a.allocate(7.0, 1).expect("allocation de A"),
                Reply::AllocGranted { .. }
            ),
            "A devait obtenir la marge entière"
        );

        // B demande la même chose : refusé, et pour la bonne raison. Le refus est
        // une RÉPONSE valide, pas une panne.
        match b.allocate(7.0, 1).expect("B doit recevoir une réponse") {
            Reply::AllocDenied { reason, .. } => {
                assert_eq!(reason, REASON_INSUFFICIENT_HEADROOM);
            }
            other => panic!("B devait être refusé, obtenu {other:?}"),
        }

        // A disparaît SANS libérer. C'est le garde RAII du serveur qui rend la
        // marge — et c'est précisément ce que ce test prouve de bout en bout, à
        // travers le client réel et non par un socket écrit à la main.
        drop(a);

        // La déconnexion n'est pas instantanément visible côté serveur : on
        // réessaie jusqu'à un délai court plutôt que de parier sur un `sleep`
        // fixe, qui rendrait ce test tantôt vert tantôt rouge selon la charge.
        let deadline = Instant::now() + Duration::from_secs(3);
        let outcome = loop {
            match b.allocate(7.0, 1).expect("nouvelle tentative de B") {
                Reply::AllocGranted { .. } => break "granted".to_string(),
                Reply::AllocDenied { reason, .. } => {
                    if Instant::now() >= deadline {
                        break format!("denied:{reason}");
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                other => panic!("réponse inattendue : {other:?}"),
            }
        };

        assert_eq!(
            outcome, "granted",
            "la marge d'un client disparu doit revenir au client suivant"
        );

        stop_server(&shutdown, handle);
    }

    #[test]
    fn test_client_rejects_mismatched_id() {
        // Le faux serveur répond un `pong` parfaitement valide, mais avec un
        // AUTRE `id` : le client ne doit pas le prendre pour sa réponse.
        let (path, handle, _dir) = start_fake_server(
            hello_line(1),
            "{\"v\":1,\"type\":\"pong\",\"id\":\"pas-le-mien\"}\n".to_string(),
        );

        let mut client = DaemonClient::connect(&path).expect("connexion au faux serveur");
        let err = match client.request(Request::Ping { id: None }) {
            Ok(reply) => panic!("un id étranger n'est pas une réponse : {reply:?}"),
            Err(err) => err,
        };

        match err {
            ClientError::Protocol(message) => assert!(
                message.contains("id de réponse inattendu"),
                "message attendu sur la corrélation, obtenu : {message}"
            ),
            other => panic!("attendu ClientError::Protocol, obtenu {other:?}"),
        }

        // Le faux serveur a pu échouer à écrire (client déjà parti) : sans
        // importance, c'est le client qu'on teste.
        let _ = handle.join();
    }

    #[test]
    fn test_client_rejects_wrong_protocol_version() {
        let (path, handle, _dir) = start_fake_server(hello_line(2), String::new());

        let err = match DaemonClient::connect(&path) {
            Ok(_) => panic!("un `hello` en version 2 doit être refusé"),
            Err(err) => err,
        };

        match err {
            ClientError::Protocol(message) => assert!(
                message.contains("version de protocole inconnue"),
                "message attendu sur la version, obtenu : {message}"
            ),
            other => panic!("attendu ClientError::Protocol, obtenu {other:?}"),
        }

        let _ = handle.join();
    }

    #[test]
    fn test_connect_with_timeout_gives_up_on_silent_server() {
        let (path, _dir) = start_silent_server();

        let started = Instant::now();
        // Pas d'`expect_err` : `DaemonClient` n'implémente pas `Debug`, et lui en
        // ajouter un pour un test serait élargir la surface publique sans raison.
        let err = match DaemonClient::connect_with_timeout(&path, Duration::from_millis(200)) {
            Ok(_) => panic!("un serveur muet ne doit pas rendre un client"),
            Err(err) => err,
        };
        let elapsed = started.elapsed();

        // Un silence est une panne de TRANSPORT, pas un refus du serveur : les
        // confondre rendrait le diagnostic d'un daemon muet impossible.
        assert!(
            matches!(err, ClientError::Io(_)),
            "un silence doit remonter en E/S, obtenu {err:?}"
        );

        // Borne LARGE (2 s pour un délai de 200 ms) : le test doit rester vert sur
        // une machine chargée. Ce qu'il interdit, c'est d'attendre les 60 s du
        // délai nominal — c'est tout le défaut qu'il verrouille.
        assert!(
            elapsed < Duration::from_secs(2),
            "le délai court devait renoncer en ~200 ms, pas en {elapsed:?}"
        );
    }
}
