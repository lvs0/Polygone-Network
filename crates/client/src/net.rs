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
use std::collections::{HashMap, VecDeque};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::identity::LocalIdentity;
use crate::msg;

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

/// Load the trust anchor (empty map = first contact, TOFU).
pub fn load_peers() -> HashMap<String, String> {
    std::fs::read_to_string(peers_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Persist the trust anchor (best-effort, chmod 600).
pub fn save_peers(peers: &HashMap<String, String>) {
    if let Ok(raw) = serde_json::to_string_pretty(peers) {
        let path = peers_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&path, raw).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

/// A network envelope, as JSON on the wire.
#[derive(serde::Serialize, serde::Deserialize)]
struct NetEnvelope {
    kind: String,
    from: String,
    to: String,
    session: String,
    seq: u64,
    #[serde(rename = "type")]
    typ: String,
    idx: u8,
    threshold: u8,
    total: u8,
    payload: Vec<u8>,
    /// Present only on the KEM envelope: ML-DSA-65 signature over the
    /// canonical message. "C'est bien Alice" — verified by the receiver
    /// before decrypt.
    #[serde(default)]
    sig: Option<Vec<u8>>,
    /// Present only on the KEM envelope: sender's ML-DSA-65 public key (hex).
    #[serde(default)]
    signer: Option<String>,
    /// Present only on the KEM envelope of a file transfer: the file name
    /// encrypted with the session key (`symmetric::encrypt` output). The
    /// relay sees only opaque bytes — the name is out-of-band.
    #[serde(default)]
    name_ct: Option<Vec<u8>>,
    /// Present only on the KEM envelope: sender clock, unix seconds. Bound
    /// into the signature (freshness) — a captured session cannot be
    /// replayed outside the ± window.
    #[serde(default)]
    ts: Option<u64>,
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
fn session_id() -> String {
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
    let session = session_id();
    let from = node_id(identity);
    let to = dest_node.to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ciphertext = output
        .ciphertext
        .clone()
        .expect("send_bytes fills ciphertext");

    // ML-DSA-65 signature over the canonical message — "c'est bien Alice".
    // The timestamp is bound in: a captured session cannot be replayed
    // outside the ± MAX_TS_SKEW_SECS window.
    let signer_pk = identity.sign_verifier()?;
    let sig = identity.sign_signer()?.sign(&canonical_bytes(
        &session,
        &from,
        &to,
        ts,
        output.kem_ct.as_bytes(),
        &ciphertext,
    ));

    // File name out-of-band: encrypted with the session key. Only the
    // recipient (who can decapsulate) reads it; the relay sees bytes.
    let name_ct = match (&output.session_key, name) {
        (Some(k), Some(n)) => Some(symmetric::encrypt(n.as_bytes(), k)?),
        _ => None,
    };

    let mut stream = connect(relay, identity).await?;

    // 1. The KEM ciphertext envelope (carries the signature + encrypted name).
    let kem_env = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: to.clone(),
        session: session.clone(),
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
    stream
        .write_all(format!("{}\n", serde_json::to_string(&kem_env)?).as_bytes())
        .await?;

    // 2. The 7 fragment envelopes.
    for (i, frag) in output.fragments.iter().enumerate() {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: from.clone(),
            to: to.clone(),
            session: session.clone(),
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
        stream
            .write_all(format!("{}\n", serde_json::to_string(&env)?).as_bytes())
            .await?;
    }

    stream.flush().await?;
    Ok(session)
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
    let mut line = String::new();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        line.clear();
        let n = tokio::time::timeout(timeout, reader.read_line(&mut line))
            .await
            .unwrap_or(Ok(0))?;
        if n == 0 {
            break;
        }
        let env: NetEnvelope = match serde_json::from_str(line.trim()) {
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
                let sig_ok = verify_grant(&env, known_peers, ghost_node);
                if sig_ok {
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

/// What a completed session produced.
pub enum Received {
    Message(String),
    File { name: String, bytes: Vec<u8> },
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

/// Build the RES grant reply for a compute request (ghost node). Pure —
/// testable without sockets. The grant is SIGNED (like messages): a
/// malicious relay cannot forge a grant for a session it has seen.
fn grant_for(req: &NetEnvelope, identity: &LocalIdentity) -> NetEnvelope {
    let from = node_id(identity);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let payload = serde_json::json!({
        "node": node_id(identity),
        "ram_mb": crate::mesh::free_ram_mb().unwrap_or(0),
        "ok": true,
    })
    .to_string()
    .into_bytes();
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
fn verify_req(env: &NetEnvelope, known_peers: &mut HashMap<String, String>, now_secs: u64) -> bool {
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

    // Anti-replay: a session that already completed is never accepted again.
    if completed.iter().any(|(sid, _)| sid == &env.session) {
        return Ok(None);
    }

    // A session belongs to one sender: key = from + session.
    let skey = format!("{}|{}", env.from, env.session);

    // Fail closed beyond the session cap; purge stale sessions.
    if sessions.len() >= MAX_SESSIONS && !sessions.contains_key(&skey) {
        return Ok(None);
    }
    if sessions.len().is_multiple_of(256) {
        sessions.retain(|_, b| now_secs.saturating_sub(b.touched) < SESSION_TTL_SECS);
    }

    // A second KEM for the same key must come from the SAME identity.
    let buf = sessions.entry(skey.clone()).or_default();
    buf.touched = now_secs;
    match env.typ.as_str() {
        "kem" => {
            if buf.kem_ct.is_some() {
                // Conflicting second KEM (different signer/from) → session
                // confusion attack: reject the whole session.
                if buf.signer.as_ref() != env.signer.as_ref() || buf.from != env.from {
                    return Ok(None);
                }
            }
            if let Ok(ct) = kem::KemCiphertext::from_bytes(&env.payload) {
                buf.kem_ct = Some(ct);
                buf.sig = env.sig.clone();
                buf.signer = env.signer.clone();
                buf.name_ct = env.name_ct.clone();
                buf.ts = env.ts;
                buf.from = env.from.clone();
                buf.to = env.to.clone();
            }
        }
        "frag" => {
            // Valid Shamir share index: 1..=7 (idx is a u8 from the wire).
            if env.idx == 0 || env.idx > 7 {
                return Ok(None);
            }
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
        _ => return Ok(None),
    };
    if now_secs.abs_diff(ts) > MAX_TS_SKEW_SECS {
        return Ok(None); // replay or clock skew — refuse
    }
    // Trust anchor: if we know this peer's signing key, it must match.
    if let Some(expected_pk) = known_peers.get(&buf.from) {
        if expected_pk != &sig.1 {
            return Ok(None); // claims to be a known peer, signs as another
        }
    }
    let verifier = match polygone_core::sign::PublicKey::from_hex(&sig.1) {
        Ok(pk) => polygone_core::sign::Verifier::from_public(pk),
        Err(_) => return Ok(None),
    };

    let shared = kem::decapsulate(&identity.kem_secret_key()?, &kem_ct)?;
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
        None => return Ok(None), // no 4-subset verifies → forged/corrupt
    };
    let plain = match symmetric::decrypt(&ciphertext, &key) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    let session = env.session.clone();
    let received = match &buf.name_ct {
        Some(ct) => {
            // Decrypt the file name with the session key (out-of-band).
            let name = symmetric::decrypt(ct, &key)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_else(|| "fichier".to_string());
            Received::File { name, bytes: plain }
        }
        None => Received::Message(String::from_utf8_lossy(&plain).to_string()),
    };
    // TOFU: learn the binding from → sign_pk on the first verified contact.
    if !known_peers.contains_key(&buf.from) {
        known_peers.insert(buf.from.clone(), sig.1.clone());
    }
    sessions.remove(&skey);
    completed.push_back((session.clone(), now_secs));
    if completed.len() > MAX_COMPLETED {
        completed.pop_front();
    }
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
    let stream = connect(relay, identity).await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
    let mut completed: VecDeque<(String, u64)> = VecDeque::new();
    // The trust anchor, loaded ONCE (persisted, learned, enforced).
    let mut known_peers = load_peers();

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
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            println!("relay déconnecté.");
            return Ok(());
        }
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
                let req_ok = verify_req(&env, &mut known_peers, now_secs);
                if !req_ok {
                    // Fail-closed: refuse, and record the failure so the
                    // reputation gate eventually refuses this borrower.
                    crate::reputation::ReputationTable::load().record(&env.from, false);
                    let mut grant = grant_for(&env, identity);
                    let mut body: serde_json::Value =
                        serde_json::from_slice(&grant.payload).unwrap_or(serde_json::json!({}));
                    body["ok"] = serde_json::Value::Bool(false);
                    body["refus"] = serde_json::Value::String("requête non authentifiée".into());
                    grant.payload = body.to_string().into_bytes();
                    let _ = writer
                        .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                        .await;
                    continue;
                }
                let mut grant = grant_for(&env, identity);
                // Reputation gate: a borrower known to fail is refused.
                let table = crate::reputation::ReputationTable::load();
                if let Some(rep) = table.nodes.get(&env.from) {
                    if rep.fail >= 3 && rep.score() < 30 {
                        let mut body: serde_json::Value =
                            serde_json::from_slice(&grant.payload).unwrap_or(serde_json::json!({}));
                        body["ok"] = serde_json::Value::Bool(false);
                        body["refus"] = serde_json::Value::String("réputation insuffisante".into());
                        grant.payload = body.to_string().into_bytes();
                        let _ = writer
                            .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                            .await;
                        continue;
                    }
                }
                let mut grant = grant_for(&env, identity);
                // Execute the task in the sandbox (RES execution layer),
                // off the event loop. Record success in the reputation.
                if let Some(output) = run_res_task(&env).await {
                    let mut body: serde_json::Value =
                        serde_json::from_slice(&grant.payload).unwrap_or(serde_json::json!({}));
                    body["output"] = serde_json::Value::String(output);
                    grant.payload = body.to_string().into_bytes();
                    crate::reputation::ReputationTable::load().record(&env.from, true);
                }
                let _ = writer
                    .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                    .await;
                println!("⬡ RES : compute accordé à {}", env.from);
                continue;
            }
        }

        let n_before = known_peers.len();
        match process_line(
            &line,
            identity,
            &mut known_peers,
            now_secs,
            &mut completed,
            &mut sessions,
        )? {
            Some((session, Received::Message(text))) => {
                // Persist a newly learned peer (TOFU trust anchor) + show it.
                announce_new_peer(&known_peers, n_before);
                save_peers(&known_peers);
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("⬡ message reçu (session {session} · 4/7 fragments)");
                println!();
                println!("{text}");
                println!();
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            Some((session, Received::File { name, bytes })) => {
                // Persist a newly learned peer (TOFU trust anchor) + show it.
                announce_new_peer(&known_peers, n_before);
                save_peers(&known_peers);
                let dir = received_dir()?;
                let path = dir.join(sanitize_name(&name));
                std::fs::write(&path, &bytes)?;
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!(
                    "⬡ fichier reçu : {} ({} octets · session {session} · 4/7 fragments)",
                    path.display(),
                    bytes.len()
                );
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
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

/// Keep only the file base name (strip any path component).
fn sanitize_name(name: &str) -> String {
    std::path::Path::new(name)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "fichier".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::LocalIdentity;

    /// Build a signed KEM envelope exactly like `send_network` does.
    fn signed_kem_json(
        session: &str,
        sender: &LocalIdentity,
        to: &str,
        ts: u64,
        output: &crate::msg::SendOutput,
        file_name: Option<&str>,
    ) -> String {
        let from = node_id(sender);
        let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
        let sig = sender.sign_signer().unwrap().sign(&canonical_bytes(
            session,
            &from,
            to,
            ts,
            output.kem_ct.as_bytes(),
            &ciphertext,
        ));
        let name_ct = match (&output.session_key, file_name) {
            (Some(k), Some(n)) => Some(symmetric::encrypt(n.as_bytes(), k).unwrap()),
            _ => None,
        };
        let env = NetEnvelope {
            kind: "fragment".into(),
            from,
            to: to.into(),
            session: session.into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: output.kem_ct.as_bytes().to_vec(),
            sig: Some(sig.as_bytes().to_vec()),
            signer: Some(sender.sign_pk_hex.clone()),
            name_ct,
            ts: Some(ts),
        };
        serde_json::to_string(&env).unwrap()
    }

    fn frag_json(session: &str, from: &str, to: &str, idx: u8, payload: &[u8]) -> String {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: from.into(),
            to: to.into(),
            session: session.into(),
            seq: idx as u64,
            typ: "frag".into(),
            idx,
            threshold: 4,
            total: 7,
            payload: payload.to_vec(),
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };
        serde_json::to_string(&env).unwrap()
    }

    /// Fixed clock for tests: passes the freshness window.
    fn test_now() -> u64 {
        1_800_000_000
    }

    #[test]
    fn canonical_bytes_are_deterministic() {
        let a = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 5]);
        let b = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 5]);
        assert_eq!(a, b);
        let c = canonical_bytes("s1", "alice", "bob", 123, &[1, 2, 3], &[4, 6]);
        assert_ne!(a, c, "different ciphertext must change the bytes");
        let d = canonical_bytes("s1", "alice", "bob", 124, &[1, 2, 3], &[4, 5]);
        assert_ne!(a, d, "different timestamp must change the bytes");
    }

    #[test]
    fn full_pipeline_signed_message_round_trip() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message réseau test", &pk).unwrap();
        let session = "test-session-1";
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        let r = process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "kem alone must not complete a session");

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            if i == 3 {
                // The 4th fragment completes the session.
                let (sid, recv) = r.expect("4/7 must complete");
                assert_eq!(sid, session);
                match recv {
                    Received::Message(text) => assert_eq!(text, "message réseau test"),
                    _ => panic!("expected a message"),
                }
            } else {
                assert!(r.is_none());
            }
        }
    }

    #[test]
    fn forged_signature_is_rejected() {
        // Eve forges a message "from Alice" — she claims Alice's node_id
        // but signs with HER own key. Bob knows Alice's signing key
        // out-of-band (the trust anchor) and must reject.
        let alice = LocalIdentity::generate();
        let eve = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("faux message d'alice", &pk).unwrap();
        let session = "forged-session";
        let bob_node = node_id(&bob);
        let alice_node = node_id(&alice);

        let mut known_peers: HashMap<String, String> = HashMap::new();
        known_peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        // KEM envelope: from = Alice's node, sig+signer = Eve's key.
        let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
        let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
            session,
            &alice_node,
            &bob_node,
            test_now(),
            output.kem_ct.as_bytes(),
            &ciphertext,
        ));
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: alice_node, // claims to be Alice
            to: bob_node.clone(),
            session: session.into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: output.kem_ct.as_bytes().to_vec(),
            sig: Some(sig.as_bytes().to_vec()),
            signer: Some(eve.sign_pk_hex.clone()),
            name_ct: None,
            ts: Some(test_now()),
        };
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        for frag in output.fragments.iter() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            assert!(r.is_none(), "a forged signature must never complete");
        }
    }

    #[test]
    fn known_peer_with_matching_key_is_accepted() {
        // The same scenario, but Alice signs with HER key: the trust
        // anchor matches, the signature verifies, the message completes.
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("vraiment alice", &pk).unwrap();
        let session = "known-peer-session";
        let bob_node = node_id(&bob);
        let alice_node = node_id(&alice);

        let mut known_peers: HashMap<String, String> = HashMap::new();
        known_peers.insert(alice_node.clone(), alice.sign_pk_hex.clone());

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            if i == 3 {
                let (_, recv) = r.expect("trusted peer must complete");
                match recv {
                    Received::Message(text) => assert_eq!(text, "vraiment alice"),
                    _ => panic!("expected a message"),
                }
                return;
            }
        }
        panic!("trusted peer session never completed");
    }

    #[test]
    fn tampered_fragment_is_detected_and_rejected() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message intègre", &pk).unwrap();
        let session = "tamper-session";
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        // Corrupt ONE fragment (flip a byte). Send exactly 4 fragments,
        // one of them corrupt: the only 4-subset reconstructs a wrong
        // secret, so the signature can never verify.
        let mut frags: Vec<_> = output.fragments.clone();
        frags[0].share[0] ^= 0xFF;
        for frag in frags.iter().take(4) {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            assert!(r.is_none(), "tampered fragments must never complete");
        }
    }

    #[test]
    fn wrong_recipient_is_ignored() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let carol = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message pour bob", &pk).unwrap();
        let session = "wrong-recipient";
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        // Envelope addressed to carol, delivered to bob.
        let kem_line =
            signed_kem_json(session, &alice, &node_id(&carol), test_now(), &output, None);
        let r = process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "envelopes not addressed to me must be ignored");
        assert!(sessions.is_empty(), "session must not be buffered");
    }

    #[test]
    fn duplicate_fragment_index_is_dropped() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message dup idx", &pk).unwrap();
        let session = "dup-idx";
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        // Send idx=1 twice + two other distinct fragments → 3 distinct
        // fragments buffered (the duplicate is dropped), session must NOT
        // complete yet.
        let f1 = &output.fragments[0];
        let f2 = &output.fragments[1];
        let f3 = &output.fragments[2];
        let f4 = &output.fragments[3];
        process_line(
            &frag_json(session, &node_id(&alice), &bob_node, f1.index, &f1.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        let r = process_line(
            &frag_json(session, &node_id(&alice), &bob_node, f1.index, &f1.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "duplicate idx must be dropped");
        process_line(
            &frag_json(session, &node_id(&alice), &bob_node, f2.index, &f2.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        process_line(
            &frag_json(session, &node_id(&alice), &bob_node, f3.index, &f3.share),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        // 3 distinct fragments → nothing completed yet, duplicate not buffered.
        let skey = format!("{}|{}", node_id(&alice), session);
        let buf = sessions.get(&skey).expect("session buffered");
        assert_eq!(buf.fragments.len(), 3, "the duplicate must not be buffered");
        // The 4th distinct fragment completes the session — normally.
        let line = frag_json(session, &node_id(&alice), &bob_node, f4.index, &f4.share);
        let (_, recv) = process_line(
            &line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap()
        .expect("4 distinct fragments must complete");
        match recv {
            Received::Message(text) => assert_eq!(text, "message dup idx"),
            _ => panic!("expected a message"),
        }
    }

    #[test]
    fn unsigned_envelope_is_rejected() {
        let bob = LocalIdentity::generate();
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("pas signé", &pk).unwrap();
        let session = "unsigned";

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        // Build a KEM envelope WITHOUT sig/signer (legacy or attacker).
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: "alice".into(),
            to: node_id(&bob),
            session: session.into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: output.kem_ct.as_bytes().to_vec(),
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(session, "alice", &bob_node, frag.index, &frag.share);
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            assert!(r.is_none(), "unsigned sessions must fail closed");
        }
    }

    #[test]
    fn full_pipeline_signed_file_with_encrypted_name() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let bytes = b"contenu secret du fichier".to_vec();
        let output = crate::msg::send_bytes(&bytes, &pk).unwrap();
        let session = "test-session-file";
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(
            session,
            &alice,
            &bob_node,
            test_now(),
            &output,
            Some("plan.txt"),
        );
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        // The relay must never see the plaintext name.
        let env: NetEnvelope = serde_json::from_str(&kem_line).unwrap();
        let ct = env.name_ct.expect("name_ct present");
        assert!(
            !ct.windows(b"plan.txt".len()).any(|w| w == b"plan.txt"),
            "file name must be opaque on the wire"
        );

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            if i == 3 {
                let (_, recv) = r.expect("file session must complete");
                match recv {
                    Received::File { name, bytes: got } => {
                        assert_eq!(name, "plan.txt");
                        assert_eq!(got, bytes);
                    }
                    _ => panic!("expected a file"),
                }
                return;
            }
        }
        panic!("file session never completed");
    }

    #[test]
    fn three_fragments_never_complete() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("trop peu de fragments", &pk).unwrap();
        let session = "test-incomplete";
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        process_line(
            &signed_kem_json(session, &alice, &bob_node, test_now(), &output, None),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter().take(3) {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            assert!(process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions
            )
            .unwrap()
            .is_none());
        }
    }

    #[test]
    fn grant_reply_routes_back_to_requester() {
        let ghost = LocalIdentity::generate();
        let req = NetEnvelope {
            kind: "fragment".into(),
            from: "borrower-node".into(),
            to: "ghost-node".into(),
            session: "res-session-1".into(),
            seq: 0,
            typ: "req".into(),
            idx: 0,
            threshold: 0,
            total: 0,
            payload: serde_json::json!({"action": "compute", "task": "bench"})
                .to_string()
                .into_bytes(),
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };

        let grant = grant_for(&req, &ghost);
        assert_eq!(grant.typ, "grant");
        assert_eq!(grant.to, "borrower-node"); // routed back to the requester
        assert_eq!(grant.session, "res-session-1"); // same session context
        assert_eq!(grant.from, node_id(&ghost)); // granted by the ghost
        let body: serde_json::Value =
            serde_json::from_slice(&grant.payload).expect("grant payload is JSON");
        assert_eq!(body["ok"], true);
        assert_eq!(body["node"], node_id(&ghost));
    }

    // ── TOFU : premier contact appris, ancre ENFORCÉE ensuite ───────────────
    // La config production charge known_peers depuis peers.json (vide au
    // premier contact) et l'apprend après un message vérifié. Ce test
    // verrouille la propriété réelle :
    //   1. premier contact (map vide) → accepté (TOFU) + binding appris ;
    //   2. message suivant du MÊME from avec une AUTRE clé → rejeté.
    #[test]
    fn tofu_learns_and_enforces_the_anchor() {
        let alice = LocalIdentity::generate(); // la vraie Alice
        let eve = LocalIdentity::generate(); // l'attaquante
        let bob = LocalIdentity::generate(); // la victime
        let pk = bob.kem_public_key().unwrap();
        let bob_node = node_id(&bob);
        let alice_node = node_id(&alice);

        let mut known_peers: HashMap<String, String> = HashMap::new(); // vide = prod fraîche
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        // 1) Eve (premier contact, from=alice, SA clé) → TOFU accepte…
        let output = crate::msg::send("premier contact", &pk).unwrap();
        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let session1 = "tofu-1";
        let ciphertext = output.ciphertext.clone().expect("ciphertext");
        let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
            session1,
            &alice_node,
            &bob_node,
            test_now(),
            output.kem_ct.as_bytes(),
            &ciphertext,
        ));
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: alice_node.clone(),
            to: bob_node.clone(),
            session: session1.into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: output.kem_ct.as_bytes().to_vec(),
            sig: Some(sig.as_bytes().to_vec()),
            signer: Some(eve.sign_pk_hex.clone()),
            name_ct: None,
            ts: Some(test_now()),
        };
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(
                session1,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let _ = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
        }
        // …et le binding from→clé d'Eve est APPRIS.
        assert_eq!(
            known_peers.get(&alice_node).map(|s| s.as_str()),
            Some(eve.sign_pk_hex.as_str()),
            "le premier contact vérifié doit être appris (TOFU)"
        );

        // 2) La vraie Alice, MÊME from, SA clé → REJETÉE (ancre enforce).
        let output2 = crate::msg::send("vraiment alice cette fois", &pk).unwrap();
        let mut sessions2: HashMap<String, SessionBuffer> = HashMap::new();
        let session2 = "tofu-2";
        let ciphertext2 = output2.ciphertext.clone().expect("ciphertext");
        let sig2 = alice.sign_signer().unwrap().sign(&canonical_bytes(
            session2,
            &alice_node,
            &bob_node,
            test_now(),
            output2.kem_ct.as_bytes(),
            &ciphertext2,
        ));
        let env2 = NetEnvelope {
            kind: "fragment".into(),
            from: alice_node.clone(),
            to: bob_node.clone(),
            session: session2.into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: output2.kem_ct.as_bytes().to_vec(),
            sig: Some(sig2.as_bytes().to_vec()),
            signer: Some(alice.sign_pk_hex.clone()),
            name_ct: None,
            ts: Some(test_now()),
        };
        process_line(
            &serde_json::to_string(&env2).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions2,
        )
        .unwrap();
        for frag in output2.fragments.iter() {
            let line = frag_json(
                session2,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions2,
            )
            .unwrap();
            assert!(
                r.is_none(),
                "une clé différente pour un from déjà ancré doit être rejetée"
            );
        }
        // Le premier from ancré est conservé (pas écrasé par le second).
        assert_eq!(
            known_peers.get(&alice_node).map(|s| s.as_str()),
            Some(eve.sign_pk_hex.as_str())
        );
    }

    #[test]
    fn replayed_session_is_rejected() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message rejoué", &pk).unwrap();
        let bob_node = node_id(&bob);

        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        // Session complète (ts valide).
        let session = "replay-1";
        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let _ = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
        }

        // La même session, rejouée → rejetée (anti-replay cache).
        let mut sessions2: HashMap<String, SessionBuffer> = HashMap::new();
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions2,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions2,
            )
            .unwrap();
            assert!(r.is_none(), "une session déjà complétée doit être rejetée");
        }
    }

    #[test]
    fn stale_timestamp_is_rejected() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("vieux message", &pk).unwrap();
        let bob_node = node_id(&bob);

        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        // Signé il y a 10 heures (hors fenêtre ±300 s).
        let old_ts = test_now() - 10 * 3600;
        let session = "stale-1";
        let kem_line = signed_kem_json(session, &alice, &bob_node, old_ts, &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            assert!(r.is_none(), "un horodatage hors fenêtre doit être rejeté");
        }
    }

    #[test]
    fn session_cap_rejects_new_sessions() {
        let bob = LocalIdentity::generate();
        let bob_node = node_id(&bob);
        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();

        // Fill the map to the cap with dummy (from|session) keys.
        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        for i in 0..MAX_SESSIONS {
            sessions.insert(format!("node{}|s{}", i % 7, i), SessionBuffer::default());
        }
        assert_eq!(sessions.len(), MAX_SESSIONS);

        // A NEW session beyond the cap → fail closed, nothing buffered.
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: "attacker".into(),
            to: bob_node,
            session: "fresh-session".into(),
            seq: 0,
            typ: "kem".into(),
            idx: 0,
            threshold: 4,
            total: 7,
            payload: vec![0u8; 32],
            sig: None,
            signer: None,
            name_ct: None,
            ts: None,
        };
        let r = process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none());
        assert_eq!(
            sessions.len(),
            MAX_SESSIONS,
            "le plafond de sessions ne doit jamais être dépassé"
        );
    }

    #[test]
    fn out_of_range_fragment_index_is_dropped() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("idx hors bornes", &pk).unwrap();
        let bob_node = node_id(&bob);

        let mut known_peers: HashMap<String, String> = HashMap::new();
        let mut completed: VecDeque<(String, u64)> = VecDeque::new();
        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        let session = "bad-idx";
        let kem_line = signed_kem_json(session, &alice, &bob_node, test_now(), &output, None);
        process_line(
            &kem_line,
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();

        // idx=200 (u8 hors 1..=7) → drop, jamais bufferisé.
        let r = process_line(
            &frag_json(session, &node_id(&alice), &bob_node, 200, &[1, 2, 3]),
            &bob,
            &mut known_peers,
            test_now(),
            &mut completed,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none());
        // 4 vrais fragments suffisent toujours à compléter (le 200 n'a rien pollué).
        for (i, frag) in output.fragments.iter().enumerate().take(4) {
            let line = frag_json(
                session,
                &node_id(&alice),
                &bob_node,
                frag.index,
                &frag.share,
            );
            let r = process_line(
                &line,
                &bob,
                &mut known_peers,
                test_now(),
                &mut completed,
                &mut sessions,
            )
            .unwrap();
            if i == 3 {
                assert!(r.is_some(), "4 fragments valides doivent compléter");
            }
        }
    }
}
