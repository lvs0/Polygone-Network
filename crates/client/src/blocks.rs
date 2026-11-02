//! blocks — magasin de blocs adressé par BLAKE3, servi par un pair, vérifié par
//! le hash (`docs/PLAN_BLOC_HASH.md`, incrément 2b-12b).
//!
//! C'est le **premier lien pair-à-pair direct** du projet : jusqu'ici il n'y
//! avait que le relais (qui ne porte que des lignes plafonnées à 64 Kio,
//! `crates/relay/src/relay.rs:39`) et le SOCKS5 de `hide.rs`. Ici, deux nœuds
//! se parlent directement, en TCP, sans état.
//!
//! # Ce que ce module n'est PAS
//!
//! * Ce n'est **pas** le protocole v1 du daemon (`daemon/src/socket.rs`) :
//!   celui-là est en NDJSON sur un socket Unix, entre un consommateur et *son*
//!   daemon. Ici : entre **pairs**, en TCP direct, sans version ni ledger.
//! * Ce n'est **pas** `Fragment::dummy_hash` (`crates/core/src/envelope.rs:51`),
//!   un pliage XOR de développement : le hash est BLAKE3 par
//!   `Receipt::hash_payload` (`crates/core/src/receipt.rs:54`).
//!
//! # Le protocole (deux messages, une connexion)
//!
//! ```text
//! demandeur → serveur : GET <64 hex minuscules>\n
//! serveur  → demandeur : HAVE <len>\n<len octets bruts>   (le bloc existe)
//! serveur  → demandeur : MISS\n                           (sinon)
//! ```
//!
//! Pas de session, pas de handshake, pas de reprise : une connexion, une
//! question, une réponse, on ferme.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Port par défaut du service de blocs.
///
/// **Décision 1 — port FIXE, pas de port dynamique annoncé dans le mesh.** Le
/// mesh (`crates/client/src/mesh.rs:23-26`) a un format d'annonce strictement
/// positionnel — `POLYGONE v1 <node_id> <relay> [<ram_mb>]` — que `discover`
/// (`:45-72`) lit champ par champ et que le test `announce_packet_format`
/// (`:180-185`) compare **au caractère**. Y ajouter un port obligerait à
/// toucher ce code qui marche, et à faire évoluer le parser en même temps que
/// l'émetteur ; un pair qui n'aurait pas la nouvelle version verrait son `ram_mb`
/// décalé. Un port fixe ne casse rien, se dit dans `--chez <ip>` (port par
/// défaut) et reste négociable par `--port` à l'écoute et `ip:port` à la
/// demande. Quand le mesh portera le port, cette constante deviendra le défaut.
pub const DEFAULT_PORT: u16 = 7643;

/// Taille maximale d'un bloc : 1 Mio (`docs/PLAN_BLOC_HASH.md` §2).
pub const MAX_BLOCK_BYTES: usize = 1024 * 1024;

/// Plafond de la ligne de requête (`GET <64 hex>` fait 69 octets).
const MAX_REQUEST_LINE: usize = 64 * 1024;

/// Délai d'E/S : un pair lent ne doit pas immobiliser un demandeur.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Connexions servies simultanément. À 1 Mio par bloc, 16 lecteurs
/// simultanés représentent déjà 16 Mio en vol : au-delà, on refuse la
/// connexion au lieu de faire tomber la machine (`relay.rs:259` fait pareil).
pub const MAX_CONNECTIONS: usize = 16;

/// Nombre de caractères hexadécimaux d'un hash BLAKE3-256.
const HASH_HEX_LEN: usize = 64;

// ── Le hash ─────────────────────────────────────────────────────────────────

/// Hash BLAKE3-256 du contenu, en hexadécimal **minuscule**.
///
/// Réutilise la seule implémentation BLAKE3 du dépôt
/// (`Receipt::hash_payload`, `crates/core/src/receipt.rs:54`) — elle est
/// publique et accessible de l'extérieur (`pub mod receipt`,
/// `crates/core/src/lib.rs:9`) : aucun changement de `crates/core` n'a été
/// nécessaire.
pub fn hash_of(bytes: &[u8]) -> String {
    hex::encode(polygone_core::receipt::Receipt::hash_payload(bytes))
}

/// Un hash n'est accepté que s'il fait **exactement** 64 caractères hex
/// minuscules.
///
/// **Décision 2 — valider AVANT tout chemin.** Ce listener accepte les
/// connexions de n'importe qui sur le LAN ; un hash reçu est une **chaîne
/// fournie par l'attaquant**. La concaténer à un chemin (`root.join(hash)`)
/// avant de la valider est la faille de traversée de tout cache adressé par
/// contenu : `GET ../../etc/passwd` lirait un fichier hors du magasin. La règle
/// ici est donc : la forme se juge d'abord, le disque ne se touche jamais avant.
fn is_valid_hash(hash: &str) -> bool {
    hash.len() == HASH_HEX_LEN
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ── Où vivent les blocs ─────────────────────────────────────────────────────

/// Racine par défaut du magasin : `~/.polygone/blocks/`.
///
/// **Décision 3 — un seul emplacement.** `~/.polygone/blocks/` remplace
/// `drive.rs` (`crates/client/src/drive.rs`), qui était **mort** : `mod drive;`
/// (`main.rs:15`) mais zéro appelant de `DriveFile` — supprimé dans ce même
/// incrément. Le chemin `~/.polygone/received/` (`net/mod.rs:1025-1031`) reste,
/// lui : il reçoit des **fichiers envoyés par un correspondant identifié** via
/// le relais, sous leur nom d'origine ; `blocks/` stocke des **contenus
/// anonymes adressés par leur hash**. Deux cycles de vie différents, deux
/// dossiers — mais un seul magasin de blocs.
pub fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".polygone")
        .join("blocks")
}

/// `(dossier de rangement, chemin final)` d'un bloc.
///
/// Le hash est déjà validé par l'appelant : découper les octets ASCII est sans
/// risque (`hash[0..2]` = dossier, `hash[2..]` = nom de fichier).
fn paths_for(root: &Path, hash: &str) -> (PathBuf, PathBuf) {
    let dir = root.join(&hash[0..2]);
    let file = dir.join(&hash[2..]);
    (dir, file)
}

/// Chemin final d'un hash **valide**, sans allocation de dossier.
fn block_path(root: &Path, hash: &str) -> Option<PathBuf> {
    if !is_valid_hash(hash) {
        return None;
    }
    Some(paths_for(root, hash).1)
}

/// Suite des temporaires d'un même processus (deux `put` concurrents ne
/// doivent pas se disputer le même nom).
fn next_tmp_seq() -> u64 {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::SeqCst)
}

// ── Écrire ──────────────────────────────────────────────────────────────────

/// Stocke `bytes` et rend son hash hex. Vois [put_in] pour le détail.
pub fn put(bytes: &[u8]) -> io::Result<String> {
    put_in(&default_root(), bytes)
}

/// Stocke `bytes` sous `root/<2hex>/<62hex>` et rend son hash.
///
/// **Décision 4 — écriture atomique DANS LE MÊME dossier.** `rename` n'est
/// atomique que dans un même système de fichiers : on écrit donc le temporaire
/// **à côté** du fichier final (`<dossier>/.tmp-<pid>-<n>`), jamais dans
/// `/tmp`, puis on renomme. Un bloc à demi écrit n'apparaît ainsi jamais sous
/// un nom valide — sinon un lecteur pourrait lire des octets tronqués sous un
/// hash qui les contredit.
pub fn put_in(root: &Path, bytes: &[u8]) -> io::Result<String> {
    if bytes.len() > MAX_BLOCK_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "bloc trop gros : {} octets (maximum {MAX_BLOCK_BYTES})",
                bytes.len()
            ),
        ));
    }

    let hash = hash_of(bytes);
    let (dir, final_path) = paths_for(root, &hash);
    std::fs::create_dir_all(&dir)?;

    let tmp = dir.join(format!(".tmp-{}-{}", std::process::id(), next_tmp_seq()));
    std::fs::write(&tmp, bytes)?;
    if let Err(e) = std::fs::rename(&tmp, &final_path) {
        let _ = std::fs::remove_file(&tmp); // pas de temporaire orphelin
        return Err(e);
    }

    // Deux blocs identiques → même hash → même fichier : le second écrase le
    // premier, aux mêmes octets. C'est la déduplication, pas une perte.
    Ok(hash)
}

// ── Lire ────────────────────────────────────────────────────────────────────

/// Lit et VÉRIFIE un bloc local. `None` si le hash est invalide, le bloc
/// absent, ou si le contenu ne correspond pas au hash.
pub fn get(hash: &str) -> Option<Vec<u8>> {
    get_in(&default_root(), hash)
}

/// Lit un bloc sous `root`, en **vérifiant le hash sur les octets lus**.
///
/// Un bloc dont le contenu contredit son nom est **supprimé** et rend `None` :
/// le garder serait empoisonner le cache, et le servir serait mentir à un pair.
pub fn get_in(root: &Path, hash: &str) -> Option<Vec<u8>> {
    // La forme se juge AVANT de construire le chemin (Décision 2).
    let final_path = block_path(root, hash)?;
    let bytes = std::fs::read(&final_path).ok()?;
    if hash_of(&bytes) != hash {
        let _ = std::fs::remove_file(&final_path);
        return None;
    }
    Some(bytes)
}

// ── Servir ──────────────────────────────────────────────────────────────────

/// Compteurs d'un service de blocs — ce que `polygone node status` affiche.
///
/// Séparé de la boucle pour que le **service** (qui a un bail à tenir, donc des
/// comptes à rendre) et l'**outil bas niveau** `polygone bloc servir` (qui n'en a
/// pas) partagent exactement le même code de service. `AtomicU64` parce que les
/// incréments viennent des threads de connexion.
#[derive(Debug, Default)]
pub struct Served {
    blocks: AtomicU64,
    bytes: AtomicU64,
    accepted: AtomicU64,
    refused: AtomicU64,
}

impl Served {
    /// Blocs effectivement servis : comptés **après** l'écriture.
    pub fn blocks(&self) -> u64 {
        self.blocks.load(Ordering::SeqCst)
    }

    /// Octets effectivement servis.
    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::SeqCst)
    }

    /// Connexions admises (un créneau leur a été donné).
    ///
    /// Avec [`refused`](Self::refused), c'est la trace **observable** d'un
    /// plafond : sans ces deux compteurs, une limite auto-imposée ne se prouve
    /// que de l'intérieur, et une limite qu'on ne peut pas observer est une
    /// limite qu'on ne peut pas tenir.
    pub fn accepted(&self) -> u64 {
        self.accepted.load(Ordering::SeqCst)
    }

    /// Connexions refusées parce que le plafond était atteint.
    pub fn refused(&self) -> u64 {
        self.refused.load(Ordering::SeqCst)
    }

    fn record(&self, len: u64) {
        self.blocks.fetch_add(1, Ordering::SeqCst);
        self.bytes.fetch_add(len, Ordering::SeqCst);
    }

    fn accept(&self) {
        self.accepted.fetch_add(1, Ordering::SeqCst);
    }

    fn refuse(&self) {
        self.refused.fetch_add(1, Ordering::SeqCst);
    }
}

/// Sert les blocs de `~/.polygone/blocks/` jusqu'à l'arrêt du processus.
pub fn serve(listener: TcpListener) -> io::Result<()> {
    serve_in(listener, default_root())
}

/// Boucle d'accept : un thread par connexion, une question, une réponse.
///
/// Passe-partout de [`serve_with`] : plafond du module, compteurs jetables,
/// aucun arrêt demandé. C'est le mode « outil bas niveau », sans bail.
pub fn serve_in(listener: TcpListener, root: PathBuf) -> io::Result<()> {
    serve_with(
        listener,
        root,
        MAX_CONNECTIONS,
        Arc::new(Served::default()),
        Arc::new(AtomicBool::new(false)),
    )
}

/// Boucle d'accept **paramétrée** : c'est celle qu'un service utilise pour tenir
/// la promesse qu'il a faite à son daemon (voir `crate::node`).
///
/// `max_connections` est le plafond d'APRÈS le bail, pas celui du module : un
/// nœud qui n'a obtenu qu'un cœur ne doit pas ouvrir seize transferts sous
/// prétexte que le module sait le faire. `stop` est vérifié à chaque acceptation ;
/// pour réveiller une acceptation bloquée, l'appelant se connecte à sa propre
/// adresse (voir `crate::node`), ce qui borne l'arrêt sans dépendre d'un délai.
/// `served` reçoit les compteurs que le service doit rendre.
pub fn serve_with(
    listener: TcpListener,
    root: PathBuf,
    max_connections: usize,
    served: Arc<Served>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    let live = Arc::new(AtomicUsize::new(0));

    for incoming in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                log::warn!("bloc: accept impossible : {e}");
                continue;
            }
        };
        if live.load(Ordering::SeqCst) >= max_connections {
            log::warn!("bloc: plafond de {max_connections} connexions atteint — refus");
            served.refuse();
            drop(stream);
            continue;
        }
        served.accept();
        live.fetch_add(1, Ordering::SeqCst);

        let root = root.clone();
        let live = live.clone();
        let served = served.clone();
        std::thread::spawn(move || {
            let _guard = ConnGuard(live);
            if let Err(e) = handle(stream, &root, &served) {
                log::debug!("bloc: connexion terminée : {e}");
            }
        });
    }
    Ok(())
}

/// Répond à une connexion. Ne touche le disque **que** si le hash est valide.
fn handle(mut stream: TcpStream, root: &Path, served: &Served) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;

    let line = match read_line(&mut stream, MAX_REQUEST_LINE)? {
        Some(l) => l,
        None => return Ok(()), // ligne absente, trop longue, ou délai dépassé
    };

    // Une seule requête acceptée : `GET <hash>`, et rien de plus.
    let mut parts = line.split_whitespace();
    let hash = match (parts.next(), parts.next(), parts.next()) {
        (Some("GET"), Some(h), None) => h,
        _ => {
            stream.write_all(b"MISS\n")?;
            return Ok(());
        }
    };

    if !is_valid_hash(hash) {
        // Aucun accès disque : `MISS`, et on ferme.
        stream.write_all(b"MISS\n")?;
        return Ok(());
    }

    match get_in(root, hash) {
        Some(bytes) => {
            writeln!(stream, "HAVE {}", bytes.len())?;
            stream.write_all(&bytes)?;
            // Compté APRÈS l'écriture : un transfert interrompu n'a pas servi.
            served.record(bytes.len() as u64);
        }
        None => stream.write_all(b"MISS\n")?,
    }
    Ok(())
}

/// Libère un créneau de connexion.
struct ConnGuard(Arc<AtomicUsize>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

// ── Demander ────────────────────────────────────────────────────────────────

/// Demande un bloc à `peer`, vérifie le hash et rend les octets.
///
/// Le pair ne décide pas de ce qu'on accepte : le hash est recalculé ici
/// (`hash_of`, BLAKE3) et un contenu qui ne correspond pas est **rejeté** —
/// rien n'est écrit par cette fonction, c'est à l'appelant de décider.
pub fn fetch(hash: &str, peer: SocketAddr) -> io::Result<Vec<u8>> {
    if !is_valid_hash(hash) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hash invalide (64 caractères hexadécimaux minuscules attendus)",
        ));
    }

    let mut stream = TcpStream::connect_timeout(&peer, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;

    writeln!(stream, "GET {hash}")?;

    let line = read_line(&mut stream, MAX_REQUEST_LINE)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "réponse absente"))?;

    if line == "MISS" {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "bloc absent chez le pair (MISS)",
        ));
    }

    let len: usize = line
        .strip_prefix("HAVE ")
        .and_then(|n| n.trim().parse().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("réponse illisible : {line}"),
            )
        })?;
    if len > MAX_BLOCK_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bloc annoncé trop gros : {len} octets"),
        ));
    }

    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes)?;

    if hash_of(&bytes) != hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "hash invalide — bloc rejeté",
        ));
    }
    Ok(bytes)
}

/// Lit une ligne terminée par `\n`, octet par octet, avec un plafond.
///
/// Rend `None` sur fin de flux, dépassement du plafond, ou délai d'attente —
/// jamais une ligne tronquée qu'on prendrait pour complète.
fn read_line(stream: &mut TcpStream, cap: usize) -> io::Result<Option<String>> {
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                if byte[0] == b'\n' {
                    return Ok(Some(
                        String::from_utf8_lossy(&buf)
                            .trim_end_matches('\r')
                            .to_string(),
                    ));
                }
                buf.push(byte[0]);
                if buf.len() > cap {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dossier temporaire propre à un test — jamais le `~/.polygone` réel.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, Ordering::SeqCst);
            let p =
                std::env::temp_dir().join(format!("polygone-blocks-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).expect("dossier temporaire");
            Self(p)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Lance un serveur sur un port éphémère et rend son adresse.
    fn spawn_server(root: PathBuf) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind éphémère");
        let addr = listener.local_addr().expect("adresse locale");
        std::thread::spawn(move || {
            let _ = serve_in(listener, root);
        });
        addr
    }

    /// Envoie une requête brute et lit la réponse ligne à ligne (faux client).
    fn raw_request(addr: SocketAddr, request: &str) -> String {
        let mut s = TcpStream::connect(addr).expect("connexion");
        s.set_read_timeout(Some(IO_TIMEOUT)).expect("délai");
        s.write_all(request.as_bytes()).expect("écriture");
        read_line(&mut s, MAX_REQUEST_LINE)
            .expect("lecture")
            .unwrap_or_default()
    }

    #[test]
    fn test_put_get_roundtrip() {
        let root = TempRoot::new();
        let data = b"le premier bloc du reseau polygone";

        let hash = put_in(root.path(), data).expect("put");
        assert_eq!(hash.len(), HASH_HEX_LEN, "hash hex de 64 caractères");
        assert_eq!(hash, hash_of(data), "le hash est celui du contenu");

        let back = get_in(root.path(), &hash).expect("get");
        assert_eq!(back, data, "mêmes octets après relecture");
    }

    #[test]
    fn test_same_content_same_hash() {
        let root = TempRoot::new();
        let data = b"deux fois le meme contenu";

        let h1 = put_in(root.path(), data).expect("premier put");
        let h2 = put_in(root.path(), data).expect("second put");
        assert_eq!(h1, h2, "contenu identique → hash identique");

        // Un seul fichier sur disque : le second écrit a écrasé le premier,
        // et aucun temporaire ne traîne.
        let dir = root.path().join(&h1[0..2]);
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("dossier du bloc")
            .map(|e| e.expect("entrée").file_name())
            .collect();
        assert_eq!(
            entries.len(),
            1,
            "un seul fichier, aucun temporaire : {entries:?}"
        );
    }

    #[test]
    fn test_get_rejects_traversal() {
        let root = TempRoot::new();
        let data = b"bloc legitime";
        let hash = put_in(root.path(), data).expect("put");

        // Chemins hostiles : jamais lus, jamais concaténés.
        let hostiles: Vec<String> = vec![
            "../../etc/passwd".into(),
            "/etc/passwd".into(),
            "..".into(),
            String::new(),
            "A".repeat(64), // majuscules refusées
            "0".repeat(63), // trop court
            "g".repeat(64), // hors [0-9a-f]
        ];
        for hostile in &hostiles {
            assert!(
                get_in(root.path(), hostile).is_none(),
                "hash hostile accepté : {hostile}"
            );
        }
        // Rien n'est entré dans le magasin en dehors du bloc légitime.
        assert_eq!(
            std::fs::read_dir(root.path()).expect("racine").count(),
            1,
            "un seul dossier créé, aucun dégât de traversée"
        );

        // Même chose côté service : `MISS`, et ça ferme.
        let addr = spawn_server(root.path().to_path_buf());
        assert_eq!(raw_request(addr, "GET ../../etc/passwd\n"), "MISS");
        assert_eq!(raw_request(addr, "GET /etc/passwd\n"), "MISS");
        assert_eq!(
            raw_request(
                addr,
                "GET AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n"
            ),
            "MISS"
        );
        // Contrôle : le bloc légitime, lui, est bien servi.
        assert_eq!(
            raw_request(addr, &format!("GET {hash}\n")),
            format!("HAVE {}", data.len())
        );
    }

    #[test]
    fn test_serve_and_fetch_roundtrip() {
        let root = TempRoot::new();
        let data = b"bloc servi par un pair, verifie par le hash".to_vec();
        let hash = put_in(root.path(), &data).expect("put");

        let addr = spawn_server(root.path().to_path_buf());
        let back = fetch(&hash, addr).expect("fetch");
        assert_eq!(back, data, "octets identiques");
        assert_eq!(hash_of(&back), hash, "hash vérifié côté demandeur");
    }

    #[test]
    fn test_fetch_rejects_tampered_block() {
        let root = TempRoot::new();
        let data = b"contenu authentique".to_vec();
        let hash = hash_of(&data);

        // Faux serveur : annonce la bonne taille, sert un octet modifié.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("adresse");
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = read_line(&mut s, MAX_REQUEST_LINE);
                let mut tampered = data.clone();
                tampered[0] ^= 0x01;
                let _ = writeln!(s, "HAVE {}", tampered.len());
                let _ = s.write_all(&tampered);
            }
        });

        let err = fetch(&hash, addr).expect_err("un bloc altéré doit être rejeté");
        assert_eq!(
            err.kind(),
            io::ErrorKind::InvalidData,
            "rejet au hash : {err}"
        );
        // Rien n'est entré dans le magasin.
        assert!(get_in(root.path(), &hash).is_none(), "aucun bloc écrit");
    }

    #[test]
    fn test_fetch_rejects_wrong_hash_in_request() {
        let root = TempRoot::new();
        let addr = spawn_server(root.path().to_path_buf());

        // Hash bien formé mais inconnu du pair : `MISS` → erreur propre.
        let unknown = "0".repeat(64);
        let err = fetch(&unknown, addr).expect_err("bloc inconnu → erreur");
        assert_eq!(err.kind(), io::ErrorKind::NotFound, "MISS attendu : {err}");
    }
}
