//! hide — Polygone Hide (HIDE-SPEC.md Phase 1): SOCKS5 proxy through the
//! blind relay.
//!
//! Two roles:
//! - **Client** (`polygone hide --via <relay> --sortie <node> -d <pk>`):
//!   a local SOCKS5 listener (127.0.0.1:9050). Every CONNECT is tunnelled
//!   through the relay to the exit node, which opens the real connection.
//! - **Exit node** (`polygone ecouter --hide`): receives `req` envelopes
//!   with an encrypted destination, opens the real TCP connection, replies
//!   a grant, then relays `stream` envelopes both ways.
//!
//! Security model (honest, see HIDE-SPEC.md):
//! - The relay sees only `kind`/`to`/`session`/sizes — never the destination
//!   (encrypted with ML-KEM) nor the content (AES-256-GCM).
//! - The exit node sees the real destination (like a Tor exit) — documented.
//! - Single hop (MVP): relay + exit node together could correlate — said
//!   out loud, no Tor-level promise.
//! - DNS : un CONNECT sur un **nom** ne le résout pas localement. Le nom
//!   part chiffré vers le nœud de sortie, qui fait la résolution réseau ;
//!   aucune requête DNS ne quitte donc le client (pas de fuite DNS).
//!   Conséquence honnête : c'est l'exit node qui voit le nom résolu, comme
//!   il voit déjà la destination — même niveau de confiance que Tor.

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use polygone_core::crypto::{kem, symmetric};

use crate::identity::LocalIdentity;
use crate::net;

/// The default SOCKS5 listen address (projected port, docs/config.md).
pub const DEFAULT_LISTEN: &str = "127.0.0.1:9050";
/// Timeout for the exit node to answer a CONNECT (grant).
const GRANT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// Timeout for the exit node's real TCP connection to the destination.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

// ─────────────────────────────────────────────────────────────────────────────
// SOCKS5 (RFC 1928) — client side
// ─────────────────────────────────────────────────────────────────────────────

/// Destination d'un CONNECT, dans la forme exacte où elle a été reçue.
///
/// La distinction compte : une [`Socks5Target::Domain`] ne doit **jamais**
/// être résolue par le client, sinon la requête DNS fuit en clair à côté du
/// tunnel. Le nom voyage tel quel dans l'enveloppe chiffrée et c'est le nœud
/// de sortie qui appelle le résolveur du système.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5Target {
    /// Nom d'hôte (ATYP 0x03) — résolu par l'exit node, pas ici.
    Domain(String),
    /// Adresse littérale (ATYP 0x01).
    Ipv4([u8; 4]),
    /// Adresse littérale (ATYP 0x04).
    Ipv6([u8; 16]),
}

impl Socks5Target {
    /// Forme textuelle `host` : nom conservé verbatim, IP rendue en notation
    /// usuelle. C'est ce qui est envoyé au nœud de sortie.
    pub fn as_connect_string(&self) -> String {
        match self {
            Self::Domain(name) => name.clone(),
            Self::Ipv4(octets) => {
                format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
            }
            Self::Ipv6(octets) => std::net::Ipv6Addr::from(*octets).to_string(),
        }
    }
}

/// A parsed SOCKS5 CONNECT request.
///
/// `host` reste la forme rendue de [`Self::target`] : les appelants existants
/// (et `net::hide_establish`) n'ont pas à changer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Connect {
    pub host: String,
    pub port: u16,
    pub target: Socks5Target,
}

impl Socks5Connect {
    /// Construit la requête depuis la destination typée ; `host` en découle.
    pub fn from_target(target: Socks5Target, port: u16) -> Self {
        let host = target.as_connect_string();
        Self { host, port, target }
    }
}

/// Parse a SOCKS5 CONNECT request. Returns the destination host/port.
///
/// RFC 1928: `VER CMD RSV ATYP DST.ADDR DST.PORT`
/// - VER=0x05, CMD=0x01 (CONNECT), RSV=0x00
/// - ATYP: 0x01 IPv4, 0x03 domain, 0x04 IPv6
pub fn parse_socks5_connect(buf: &[u8]) -> Result<Socks5Connect> {
    if buf.len() < 4 || buf[0] != 0x05 {
        anyhow::bail!("pas un handshake SOCKS5 (VER != 5)");
    }
    if buf[1] != 0x01 {
        anyhow::bail!("seul CONNECT (0x01) est supporté");
    }
    if buf[2] != 0x00 {
        anyhow::bail!("RSV doit être 0x00");
    }
    let atyp = buf[3];
    match atyp {
        0x01 => {
            if buf.len() < 10 {
                anyhow::bail!("adresse IPv4 tronquée");
            }
            let target = Socks5Target::Ipv4([buf[4], buf[5], buf[6], buf[7]]);
            let port = u16::from_be_bytes([buf[8], buf[9]]);
            Ok(Socks5Connect::from_target(target, port))
        }
        0x03 => {
            if buf.len() < 5 {
                anyhow::bail!("domaine tronqué");
            }
            // La longueur est portée par un octet : > 255 est impossible à
            // encoder, il suffit donc de refuser le vide et l'UTF-8 invalide.
            let len = buf[4] as usize;
            if len == 0 {
                anyhow::bail!("domaine vide");
            }
            if buf.len() < 5 + len + 2 {
                anyhow::bail!("domaine tronqué (len {len})");
            }
            // Pas de `from_utf8_lossy` ici : un nom non UTF-8 est refusé
            // plutôt que remplacé par des caractères de substitution, qui
            // enverraient l'exit node vers un hôte qui n'a jamais été demandé.
            let name = std::str::from_utf8(&buf[5..5 + len])
                .map_err(|_| anyhow::anyhow!("domaine non UTF-8"))?
                .to_string();
            let port = u16::from_be_bytes([buf[5 + len], buf[6 + len]]);
            Ok(Socks5Connect::from_target(Socks5Target::Domain(name), port))
        }
        0x04 => {
            if buf.len() < 22 {
                anyhow::bail!("adresse IPv6 tronquée");
            }
            let octets = <[u8; 16]>::try_from(&buf[4..20])?;
            let target = Socks5Target::Ipv6(octets);
            let port = u16::from_be_bytes([buf[20], buf[21]]);
            Ok(Socks5Connect::from_target(target, port))
        }
        other => anyhow::bail!("ATYP inconnu : 0x{other:02x}"),
    }
}

/// The SOCKS5 success reply (bound addr 0.0.0.0:0).
pub fn socks5_success() -> Vec<u8> {
    vec![0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]
}

/// The SOCKS5 failure reply (general failure = 0x01).
pub fn socks5_failure() -> Vec<u8> {
    vec![0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0]
}

// ─────────────────────────────────────────────────────────────────────────────
// Circuit — planification multi-hop (NON transportée aujourd'hui)
// ─────────────────────────────────────────────────────────────────────────────
//
// `Circuit` décrit un chemin que le protocole du relay ne sait pas encore
// emprunter : à ce jour `net::hide_establish` ouvre UN saut (client → relay →
// exit). Ce type sert à préparer et valider un chemin (2 ou 3 nœuds, sans
// doublon, clés bien formées) sans prétendre que le chaînage est actif. Le
// jour où le protocole relayera des enveloppes imbriquées, c'est cette
// validation qui sera réutilisée — pas un nouveau format.

/// Nombre maximum de sauts d'un circuit (au-delà, latence et corrélation
/// deviennent pires que le gain d'anonymat).
pub const MAX_HOPS: usize = 3;

/// Un saut : un nœud joignable et sa clé publique (hex).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hop {
    pub node_id: String,
    pub public_key_hex: String,
}

/// Un chemin ordonné du premier relais vers le nœud de sortie.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Circuit {
    pub hops: Vec<Hop>,
}

impl Circuit {
    /// Construit un circuit et le valide immédiatement.
    pub fn new(hops: Vec<Hop>) -> Result<Self> {
        let circuit = Self { hops };
        circuit.validate()?;
        Ok(circuit)
    }

    /// Refuse : circuit vide, trop long, nœud dupliqué, identifiant ou clé
    /// absents, clé qui n'est pas un hexadécimal de longueur paire.
    /// Aucune taille de clé n'est imposée ici : la clé d'un nœud peut être
    /// une clé KEM comme une clé de test, et le format exact appartient au
    /// transport, pas au plan.
    pub fn validate(&self) -> Result<()> {
        if self.hops.is_empty() {
            anyhow::bail!("circuit vide");
        }
        if self.hops.len() > MAX_HOPS {
            anyhow::bail!(
                "circuit trop long : {} sauts (max {MAX_HOPS})",
                self.hops.len()
            );
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for hop in &self.hops {
            let id = hop.node_id.trim();
            if id.is_empty() {
                anyhow::bail!("nœud sans node_id");
            }
            if !seen.insert(id) {
                anyhow::bail!("nœud dupliqué dans le circuit : {id}");
            }
            let key = hop.public_key_hex.trim();
            if key.is_empty() {
                anyhow::bail!("nœud {id} sans clé publique");
            }
            if key.len() % 2 != 0 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
                anyhow::bail!("clé publique du nœud {id} n'est pas un hexadécimal pair");
            }
        }
        Ok(())
    }

    /// Le même circuit parcouru de la sortie vers l'entrée — l'ordre dans
    /// lequel un chiffrement en couches doit être appliqué.
    pub fn reversed(&self) -> Circuit {
        Circuit {
            hops: self.hops.iter().rev().cloned().collect(),
        }
    }

    /// Le nœud de sortie (dernier saut), seul à voir la destination réelle.
    pub fn exit(&self) -> Option<&Hop> {
        self.hops.last()
    }

    pub fn len(&self) -> usize {
        self.hops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }
}

/// Analyse un chemin `node_id:clef,node_id:clef` et affiche le plan.
///
/// N'ouvre **aucune** connexion et ne démarre pas de proxy : c'est le mode
/// « dry-run » du multi-hop, branché sur `polygone hide --circuit`. Le
/// transport reste mono-saut, et la sortie de cette fonction le dit.
pub fn plan_circuit(spec: &str) -> Result<()> {
    let hops: Vec<Hop> = spec
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (node_id, key) = entry
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("saut « {entry} » : format attendu node_id:clef"))?;
            Ok(Hop {
                node_id: node_id.trim().to_string(),
                public_key_hex: key.trim().to_string(),
            })
        })
        .collect::<Result<Vec<Hop>>>()?;

    let circuit = Circuit::new(hops)?;
    if circuit.is_empty() {
        anyhow::bail!("circuit vide");
    }

    println!(
        "⬡ Hide — circuit planifié ({} saut(s) max {MAX_HOPS})",
        circuit.len()
    );
    for (index, hop) in circuit.hops.iter().enumerate() {
        let preview = &hop.public_key_hex[..hop.public_key_hex.len().min(12)];
        println!("  {}. {}  clé {preview}…", index + 1, hop.node_id);
    }
    if let Some(exit) = circuit.exit() {
        println!(
            "  sortie : {} — le seul nœud qui verra la destination",
            exit.node_id
        );
    }
    let reversed = circuit.reversed();
    let order: Vec<&str> = reversed
        .hops
        .iter()
        .map(|hop| hop.node_id.as_str())
        .collect();
    println!("  ordre de chiffrement : {}", order.join(" → "));

    println!();
    println!("  Transport : un seul saut aujourd'hui. Le chaînage n'est pas branché ;");
    println!("  ce plan valide le format, les doublons et l'ordre, rien de plus.");
    Ok(())
}

/// Read and answer the SOCKS5 greeting (`05 <nmethods> <methods>`).
async fn socks5_greeting<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin>(
    reader: &mut R,
    writer: &mut W,
) -> Result<()> {
    let mut head = [0u8; 2];
    reader.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        anyhow::bail!("SOCKS5 : VER != 5");
    }
    let nmethods = head[1] as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;
    // Reply: no authentication required (0x00).
    writer.write_all(&[0x05, 0x00]).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a full SOCKS5 CONNECT request (bounded to 262 bytes: 4 + 255 + 2).
async fn read_connect_request<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut head = [0u8; 4];
    reader.read_exact(&mut head).await?;
    let atyp = head[3];
    let addr_len = match atyp {
        0x01 => 4,
        0x03 => {
            let mut len = [0u8; 1];
            reader.read_exact(&mut len).await?;
            len[0] as usize
        }
        0x04 => 16,
        _ => anyhow::bail!("SOCKS5 : ATYP inconnu 0x{atyp:02x}"),
    };
    let mut rest = vec![0u8; addr_len + 2];
    reader.read_exact(&mut rest).await?;
    let mut buf = Vec::with_capacity(4 + rest.len());
    buf.extend_from_slice(&head);
    if atyp == 0x03 {
        buf.push(addr_len as u8);
    }
    buf.extend_from_slice(&rest);
    Ok(buf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Client : SOCKS5 listener → tunnel
// ─────────────────────────────────────────────────────────────────────────────

/// Run the client: SOCKS5 listener on `listen`, each CONNECT tunnelled to
/// `exit_node` through `relay`, using `exit_pk_hex` (ML-KEM public key of
/// the exit node) for the encrypted destination.
pub async fn serve(
    relay: &str,
    exit_node: &str,
    exit_pk_hex: &str,
    listen: &str,
    identity: LocalIdentity,
) -> Result<()> {
    let exit_pk = kem::KemPublicKey::from_hex(exit_pk_hex)?;
    let listener = TcpListener::bind(listen).await?;
    println!("⬡ hide : SOCKS5 sur {listen} — tunnel → {exit_node} via relay {relay}");
    println!("  (Ctrl-C pour arrêter)");

    loop {
        let (sock, _peer) = listener.accept().await?;
        let relay = relay.to_string();
        let exit_node = exit_node.to_string();
        let exit_pk = exit_pk.clone();
        let identity = identity.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_socks_connection(sock, &relay, &exit_node, &exit_pk, &identity).await
            {
                println!("  ✖ hide : {e}");
            }
        });
    }
}

/// Handle one SOCKS5 client connection end to end.
async fn handle_socks_connection(
    mut sock: TcpStream,
    relay: &str,
    exit_node: &str,
    exit_pk: &kem::KemPublicKey,
    identity: &LocalIdentity,
) -> Result<()> {
    let (mut r, mut w) = sock.split();
    socks5_greeting(&mut r, &mut w).await?;
    let req_buf = read_connect_request(&mut r).await?;
    let target = parse_socks5_connect(&req_buf)?;

    // Open the tunnel through the relay to the exit node.
    let channel = match net::hide_establish(
        relay,
        exit_node,
        exit_pk,
        identity,
        &target.host,
        target.port,
        GRANT_TIMEOUT,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            // The application is waiting for an answer — tell it the
            // connection failed (RFC 1928 REP 0x01, general failure).
            let _ = w.write_all(&socks5_failure()).await;
            anyhow::bail!("tunnel refusé : {e}");
        }
    };

    // Tunnel established — tell the application it can go.
    w.write_all(&socks5_success()).await?;
    w.flush().await?;
    println!(
        "  ⬡ tunnel ouvert : {}:{} → {exit_node} (session {})",
        target.host, target.port, channel.session
    );

    // Bidirectional relay:
    //   app → (encrypt) → relay → exit node
    //   app ← (decrypt) ← relay ← exit node
    let (mut app_r, mut app_w) = sock.into_split();
    let relay_writer = Arc::new(Mutex::new(channel.writer));
    let key = channel.key.clone();
    let session = channel.session.clone();
    let to = exit_node.to_string();
    let from = net::node_id(identity);

    // Task 1: app → exit node (read the SOCKS client, encrypt, forward).
    let up_writer = relay_writer.clone();
    let up_key = key.clone();
    let up_session = session.clone();
    let up_to = to.clone();
    let up_from = from.clone();
    let up = tokio::spawn(async move {
        let mut buf = vec![0u8; net::STREAM_CHUNK];
        loop {
            let n = match app_r.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let ct = match net::hide_encrypt_chunk(&buf[..n], &up_key) {
                Ok(c) => c,
                Err(_) => break,
            };
            let env = net::stream_envelope(&up_session, &up_from, &up_to, &ct);
            let line = match net::envelope_line(&env) {
                Ok(l) => l,
                Err(_) => break,
            };
            let mut w = up_writer.lock().await;
            if w.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
        // EOF marker: tell the exit node the client side is done.
        let env = net::stream_envelope(&up_session, &up_from, &up_to, &[]);
        if let Ok(line) = net::envelope_line(&env) {
            let mut w = up_writer.lock().await;
            let _ = w.write_all(line.as_bytes()).await;
        }
    });

    // Task 2: exit node → app (read stream envelopes, decrypt, write).
    let mut down_reader = channel.reader;
    let down_key = key.clone();
    let down_session = session.clone();
    let down = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            let n = match down_reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if n == 0 {
                break;
            }
            let env: net::NetEnvelope = match serde_json::from_str(line.trim()) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if env.typ != "stream" || env.session != down_session {
                continue;
            }
            match net::hide_decrypt_chunk(&env.payload, &down_key) {
                Ok(Some(data)) => {
                    if app_w.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break, // EOF from the exit side
                Err(_) => break,
            }
        }
    });

    let _ = tokio::join!(up, down);
    println!("  ⬡ tunnel fermé (session {})", channel.session);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Exit node : receive hide requests, open the real connection, relay streams
// ─────────────────────────────────────────────────────────────────────────────

/// The exit node loop: connect to the relay, answer `req` hide envelopes by
/// opening the real TCP connection, then relay `stream` envelopes both ways.
pub async fn exit_listen(relay: &str, identity: &LocalIdentity) -> Result<()> {
    let stream = net::connect_relay(relay, identity).await?;
    let (reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(writer));
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut known_peers = net::load_peers();
    // Active tunnels: session → (session key, real-connection write half).
    let mut tunnels: HashMap<String, (symmetric::SessionKey, OwnedWriteHalf)> = HashMap::new();

    println!(
        "⬡ hide : nœud de sortie actif via relay {relay} — node {}",
        net::node_id(identity)
    );
    println!("  (Ctrl-C pour arrêter)");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            println!("relay déconnecté.");
            return Ok(());
        }
        if line.starts_with("HELLO_DENIED") {
            println!("✖ ce node_id est déjà connecté au relay (slot pris).");
            return Ok(());
        }
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let env: net::NetEnvelope = match serde_json::from_str(line.trim()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Incoming data for an active tunnel → decrypt, write to the target.
        if env.typ == "stream" {
            if let Some((key, tcp_w)) = tunnels.get_mut(&env.session) {
                match net::hide_decrypt_chunk(&env.payload, key) {
                    Ok(Some(data)) => {
                        if tcp_w.write_all(&data).await.is_err() {
                            // The target closed — drop the tunnel.
                            tunnels.remove(&env.session);
                        }
                    }
                    Ok(None) => {
                        // EOF from the client → close the target side.
                        tunnels.remove(&env.session);
                    }
                    Err(_) => {
                        // Corrupted chunk → fail closed, drop the tunnel.
                        tunnels.remove(&env.session);
                    }
                }
            }
            continue;
        }

        // A hide request addressed to us.
        if env.typ == "req" && env.to == net::node_id(identity) {
            // Authenticate the sender (signed request, fresh timestamp,
            // trust anchor). Only provable senders get a tunnel.
            if !net::verify_req(&env, &mut known_peers, now_secs) {
                println!("  ✖ hide : requête non authentifiée de {}", env.from);
                continue;
            }
            net::save_peers(&known_peers);

            // Decapsulate: first CT_SIZE bytes are the ML-KEM ciphertext,
            // the rest is the AES-GCM-encrypted destination.
            let ct = match kem::KemCiphertext::from_bytes(&env.payload[..kem::CT_SIZE]) {
                Ok(c) => c,
                Err(_) => {
                    println!("  ✖ hide : KEM ciphertext invalide de {}", env.from);
                    continue;
                }
            };
            let shared = match kem::decapsulate(&identity.kem_secret_key()?, &ct) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let key = symmetric::SessionKey::derive_from_secret(&shared);
            let plain = match symmetric::decrypt(&env.payload[kem::CT_SIZE..], &key) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let body: serde_json::Value = match serde_json::from_slice(&plain) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let host = match body.get("host").and_then(|h| h.as_str()) {
                Some(h) => h.to_string(),
                None => continue,
            };
            let port = match body.get("port").and_then(|p| p.as_u64()) {
                Some(p) if p <= u16::MAX as u64 => p as u16,
                _ => continue,
            };

            // Open the real connection. Fail-closed: a refused connection
            // is answered with an encrypted `ok:false` grant.
            let target = match tokio::time::timeout(
                CONNECT_TIMEOUT,
                TcpStream::connect((host.as_str(), port)),
            )
            .await
            {
                Ok(Ok(t)) => t,
                _ => {
                    println!(
                        "  ✖ hide : connexion impossible vers {host}:{port} (depuis {})",
                        env.from
                    );
                    let grant_payload = match symmetric::encrypt(br#"{"ok":false}"#, &key) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let grant = net::grant_envelope(
                        &env.session,
                        &net::node_id(identity),
                        &env.from,
                        &grant_payload,
                    );
                    if let Ok(line) = net::envelope_line(&grant) {
                        let mut w = writer.lock().await;
                        let _ = w.write_all(line.as_bytes()).await;
                    }
                    continue;
                }
            };

            // Grant: encrypted with the session key — only the requester can
            // read it (the relay cannot forge it without the key).
            let grant_payload = match symmetric::encrypt(br#"{"ok":true}"#, &key) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let grant = net::grant_envelope(
                &env.session,
                &net::node_id(identity),
                &env.from,
                &grant_payload,
            );
            let line = net::envelope_line(&grant)?;
            {
                let mut w = writer.lock().await;
                w.write_all(line.as_bytes()).await?;
            }

            println!(
                "  ⬡ hide : tunnel accordé à {} → {}:{} (session {})",
                env.from, host, port, env.session
            );

            // Split the real connection; keep the write half for incoming
            // stream data, spawn a task for the return direction.
            let (mut tcp_r, tcp_w) = target.into_split();
            tunnels.insert(env.session.clone(), (key.clone(), tcp_w));

            // Task: real target → encrypt → stream envelopes back to client.
            let up_writer = writer.clone();
            let up_key = key.clone();
            let up_session = env.session.clone();
            let up_from = net::node_id(identity);
            let up_to = env.from.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; net::STREAM_CHUNK];
                loop {
                    let n = match tcp_r.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let ct = match net::hide_encrypt_chunk(&buf[..n], &up_key) {
                        Ok(c) => c,
                        Err(_) => break,
                    };
                    let env = net::stream_envelope(&up_session, &up_from, &up_to, &ct);
                    let line = match net::envelope_line(&env) {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    let mut w = up_writer.lock().await;
                    if w.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
                // EOF: tell the client the target side is done.
                let env = net::stream_envelope(&up_session, &up_from, &up_to, &[]);
                if let Ok(line) = net::envelope_line(&env) {
                    let mut w = up_writer.lock().await;
                    let _ = w.write_all(line.as_bytes()).await;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks5_parse_ipv4() {
        // CONNECT 1.2.3.4:8080
        let req = [0x05, 0x01, 0x00, 0x01, 1, 2, 3, 4, 0x1f, 0x90];
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(c.host, "1.2.3.4");
        assert_eq!(c.port, 8080);
    }

    #[test]
    fn socks5_parse_domain() {
        // CONNECT example.com:443
        let mut req = vec![0x05, 0x01, 0x00, 0x03, 11];
        req.extend_from_slice(b"example.com");
        req.extend_from_slice(&[0x01, 0xbb]); // 443
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(c.host, "example.com");
        assert_eq!(c.port, 443);
    }

    #[test]
    fn socks5_reject_bad_ver() {
        assert!(parse_socks5_connect(&[0x04, 0x01, 0x00, 0x01]).is_err());
    }

    #[test]
    fn socks5_reject_bind() {
        // CMD=0x02 (BIND) is not supported.
        assert!(parse_socks5_connect(&[0x05, 0x02, 0x00, 0x01]).is_err());
    }

    #[test]
    fn socks5_replies_have_correct_shape() {
        assert_eq!(socks5_success().len(), 10);
        assert_eq!(socks5_success()[1], 0x00);
        assert_eq!(socks5_failure()[1], 0x01);
    }

    #[test]
    fn socks5_domain_target_is_kept_unresolved() {
        let mut req = vec![0x05, 0x01, 0x00, 0x03, 11];
        req.extend_from_slice(b"example.com");
        req.extend_from_slice(&[0x01, 0xbb]);
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(
            c.target,
            Socks5Target::Domain("example.com".to_string()),
            "le nom doit rester un nom : le résoudre ici serait la fuite DNS"
        );
        assert_eq!(c.host, "example.com");
    }

    #[test]
    fn socks5_ipv4_target_is_typed_and_rendered() {
        let req = [0x05, 0x01, 0x00, 0x01, 10, 0, 0, 7, 0x00, 0x50];
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(c.target, Socks5Target::Ipv4([10, 0, 0, 7]));
        assert_eq!(c.target.as_connect_string(), "10.0.0.7");
        assert_eq!(c.host, "10.0.0.7");
        assert_eq!(c.port, 80);
    }

    #[test]
    fn socks5_ipv6_target_is_typed_and_rendered() {
        let mut req = vec![0x05, 0x01, 0x00, 0x04];
        req.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
        req.extend_from_slice(&[0x01, 0xbb]);
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(
            c.target,
            Socks5Target::Ipv6(std::net::Ipv6Addr::LOCALHOST.octets())
        );
        assert_eq!(c.target.as_connect_string(), "::1");
        assert_eq!(c.port, 443);
    }

    #[test]
    fn socks5_rejects_empty_domain() {
        let req = [0x05, 0x01, 0x00, 0x03, 0x00, 0x00, 0x50];
        assert!(parse_socks5_connect(&req).is_err());
    }

    #[test]
    fn socks5_rejects_non_utf8_domain() {
        let mut req = vec![0x05, 0x01, 0x00, 0x03, 2];
        req.extend_from_slice(&[0xff, 0xfe]);
        req.extend_from_slice(&[0x00, 0x50]);
        assert!(parse_socks5_connect(&req).is_err());
    }

    #[test]
    fn socks5_rejects_truncated_domain() {
        // len annonce 11 octets, 3 seulement sont présents.
        let mut req = vec![0x05, 0x01, 0x00, 0x03, 11];
        req.extend_from_slice(b"exa");
        assert!(parse_socks5_connect(&req).is_err());
    }

    fn hop(id: &str) -> Hop {
        Hop {
            node_id: id.to_string(),
            public_key_hex: "00ff11ee22dd33cc".to_string(),
        }
    }

    #[test]
    fn circuit_two_hops_is_valid() {
        let c = Circuit::new(vec![hop("entry"), hop("exit")]).unwrap();
        assert_eq!(c.len(), 2);
        assert!(!c.is_empty());
        assert_eq!(c.exit().map(|h| h.node_id.as_str()), Some("exit"));
    }

    #[test]
    fn circuit_rejects_more_than_max_hops() {
        let hops = vec![hop("a"), hop("b"), hop("c"), hop("d")];
        assert!(Circuit::new(hops).is_err());
    }

    #[test]
    fn circuit_rejects_duplicate_node() {
        assert!(Circuit::new(vec![hop("same"), hop("same")]).is_err());
    }

    #[test]
    fn circuit_rejects_empty() {
        assert!(Circuit::new(vec![]).is_err());
    }

    #[test]
    fn circuit_rejects_bad_hex_key() {
        let bad = Hop {
            node_id: "n".into(),
            public_key_hex: "zzz".into(),
        };
        assert!(Circuit::new(vec![bad]).is_err());

        let odd = Hop {
            node_id: "n".into(),
            public_key_hex: "abc".into(),
        };
        assert!(Circuit::new(vec![odd]).is_err());

        let empty = Hop {
            node_id: "n".into(),
            public_key_hex: String::new(),
        };
        assert!(Circuit::new(vec![empty]).is_err());
    }

    #[test]
    fn circuit_rejects_blank_node_id() {
        let blank = Hop {
            node_id: "   ".into(),
            public_key_hex: "00ff".into(),
        };
        assert!(Circuit::new(vec![blank]).is_err());
    }

    #[test]
    fn circuit_reversed_puts_exit_last() {
        let c = Circuit::new(vec![hop("entry"), hop("middle"), hop("exit")]).unwrap();
        let r = c.reversed();
        assert_eq!(r.hops[0].node_id, "exit");
        assert_eq!(r.hops[2].node_id, "entry");
        // L'aller-retour redonne l'original.
        assert_eq!(r.reversed(), c);
        assert_eq!(r.exit().map(|h| h.node_id.as_str()), Some("entry"));
    }
}
