//! net — real network transport over the blind relay (plane 2 of the spec).
//!
//! ```text
//! Alice (sender)                        Bob (receiver)
//!   │                                    │
//!   │  HELLO <node_id>                   │  HELLO <node_id>
//!   ├──────────────────────► relay ◄─────┤
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
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::identity::LocalIdentity;
use crate::msg;

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
    /// canonical message (session || from || to || kem_ct || ciphertext).
    /// "C'est bien Alice" — verified by the receiver before decrypt.
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
}

/// The node id of an identity: first 16 hex chars of the KEM public key.
pub fn node_id(identity: &LocalIdentity) -> String {
    identity.kem_pk_hex.chars().take(16).collect()
}

/// Canonical bytes signed by the sender and verified by the receiver.
///
/// Deterministic: the receiver rebuilds the exact same bytes from the
/// session metadata + the reconstructed ciphertext (Shamir rebuilds the
/// same secret from any 4-of-7 fragments), so verification needs no
/// additional fields on the wire.
pub fn canonical_bytes(
    session: &str,
    from: &str,
    to: &str,
    kem_ct: &[u8],
    ciphertext: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(
        64 + session.len() + from.len() + to.len() + kem_ct.len() + ciphertext.len(),
    );
    out.extend_from_slice(b"polygone-net-v1\0");
    out.extend_from_slice(session.as_bytes());
    out.push(0);
    out.extend_from_slice(from.as_bytes());
    out.push(0);
    out.extend_from_slice(to.as_bytes());
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
async fn connect(relay: &str, identity: &LocalIdentity) -> Result<TcpStream> {
    let mut stream = TcpStream::connect(relay).await?;
    stream
        .write_all(format!("HELLO {}\n", node_id(identity)).as_bytes())
        .await?;
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
    let ciphertext = output
        .ciphertext
        .clone()
        .expect("send_bytes fills ciphertext");

    // ML-DSA-65 signature over the canonical message — "c'est bien Alice".
    let signer_pk = identity.sign_verifier()?;
    let sig = identity.sign_signer()?.sign(&canonical_bytes(
        &session,
        &from,
        &to,
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
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let body = serde_json::json!({"action": "compute", "task": task});
    borrow_request(relay, ghost_node, body, identity, timeout).await
}

/// Send a WASM module to a ghost node for sandboxed execution.
pub async fn borrow_wasm(
    relay: &str,
    ghost_node: &str,
    wasm: &[u8],
    identity: &LocalIdentity,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let body = serde_json::json!({
        "action": "wasm",
        "wasm": base64_encode(wasm),
    });
    borrow_request(relay, ghost_node, body, identity, timeout).await
}

/// Minimal base64 encoder (no external dep).
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
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
async fn borrow_request(
    relay: &str,
    ghost_node: &str,
    body: serde_json::Value,
    identity: &LocalIdentity,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let mut stream = connect(relay, identity).await?;
    let from = node_id(identity);

    let req = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: ghost_node.to_string(),
        session: session_id(),
        seq: 0,
        typ: "req".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: body.to_string().into_bytes(),
        sig: None,
        signer: None,
        name_ct: None,
    };
    stream
        .write_all(format!("{}\n", serde_json::to_string(&req)?).as_bytes())
        .await?;
    stream.flush().await?;

    // Listen for the grant reply on the same connection.
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
            return Ok(Some(String::from_utf8_lossy(&env.payload).to_string()));
        }
    }
    Ok(None)
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
    /// Seen fragment indices — duplicates are dropped (anti-DoS).
    seen_idx: Vec<u8>,
    fragments: Vec<shamir::Fragment>,
}

/// What a completed session produced.
pub enum Received {
    Message(String),
    File { name: String, bytes: Vec<u8> },
}

/// Build the RES grant reply for a compute request (ghost node). Pure —
/// testable without sockets.
fn grant_for(req: &NetEnvelope, identity: &LocalIdentity) -> NetEnvelope {
    NetEnvelope {
        kind: "fragment".into(),
        from: node_id(identity),
        to: req.from.clone(),
        session: req.session.clone(),
        seq: 0,
        typ: "grant".into(),
        idx: 0,
        threshold: 0,
        total: 0,
        payload: serde_json::json!({
            "node": node_id(identity),
            "ram_mb": crate::mesh::free_ram_mb().unwrap_or(0),
            "ok": true,
        })
        .to_string()
        .into_bytes(),
        sig: None,
        signer: None,
        name_ct: None,
    }
}

/// Run the task from a RES request inside the systemd sandbox and return
/// the output. Empty input = no execution (grant without output).
/// If the request carries WASM bytes, they run in the wasmi sandbox instead.
fn run_res_task(req: &NetEnvelope) -> Option<String> {
    let body: serde_json::Value = serde_json::from_slice(&req.payload).ok()?;
    if let Some(wasm_b64) = body.get("wasm").and_then(|w| w.as_str()) {
        // WASM execution (Phase 8): decode, run in wasmi, return output.
        let wasm = base64_decode(wasm_b64)?;
        match crate::exec::run_wasm(&wasm, std::time::Duration::from_secs(20)) {
            Ok(out) => Some(out),
            Err(e) => Some(format!("[erreur wasm] {e}")),
        }
    } else {
        let task = body.get("task")?.as_str()?;
        if task.trim().is_empty() {
            return None;
        }
        match crate::exec::run_sandboxed(task, 256, 50, std::time::Duration::from_secs(30)) {
            Ok(out) => Some(out),
            Err(e) => Some(format!("[erreur sandbox] {e}")),
        }
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
        let v = match TABLE.iter().position(|&t| t == c) {
            Some(i) => i as u32,
            None => return None,
        };
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
/// Fail-closed: an unverifiable signature, a missing signature, an
/// envelope not addressed to us, or a duplicate fragment index all stop
/// the session from completing.
///
/// `known_peers` binds a node_id to its expected ML-DSA public key
/// (`from` → sign_pk_hex). If the sender's `from` is known and the
/// envelope's `signer` does not match, the session is rejected: the
/// signature proves possession of the signing key, the binding proves it
/// is really that peer. An empty map = first-contact trust (TOFU): the
/// signature is self-consistent, the binding is learned.
fn process_line(
    line: &str,
    identity: &LocalIdentity,
    known_peers: &HashMap<String, String>,
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

    let buf = sessions.entry(env.session.clone()).or_default();
    match env.typ.as_str() {
        "kem" => match kem::KemCiphertext::from_bytes(&env.payload) {
            Ok(ct) => {
                buf.kem_ct = Some(ct);
                buf.sig = env.sig.clone();
                buf.signer = env.signer.clone();
                buf.name_ct = env.name_ct.clone();
                buf.from = env.from.clone();
                buf.to = env.to.clone();
            }
            Err(_) => {}
        },
        "frag" => {
            // Duplicate fragment index → drop (a second idx=1 corrupts
            // reconstruction; refusing it is anti-DoS).
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

    // ── Completion: verify, then decrypt. Fail closed on either. ──────
    let kem_ct = buf.kem_ct.clone().expect("checked");
    let sig = match (buf.sig.clone(), buf.signer.clone()) {
        (Some(s), Some(pk_hex)) => (s, pk_hex),
        _ => return Ok(None), // no signature → not from anyone provable
    };
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
        let canonical = canonical_bytes(&env.session, &buf.from, &buf.to, kem_ct.as_bytes(), &cand);
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
    sessions.remove(&session);
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

    println!(
        "⬡ en écoute via relay {} — node {}",
        relay,
        node_id(identity)
    );
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

        // RES compute requests: answer with a grant (ghost node).
        // Only answer requests addressed to this node (anti-spoofing).
        if let Ok(env) = serde_json::from_str::<NetEnvelope>(line.trim()) {
            if env.typ == "req" && compute && env.to == node_id(identity) {
                let mut grant = grant_for(&env, identity);
                // Execute the task in the systemd sandbox (RES execution layer).
                if let Some(output) = run_res_task(&env) {
                    let mut body: serde_json::Value =
                        serde_json::from_slice(&grant.payload).unwrap_or(serde_json::json!({}));
                    body["output"] = serde_json::Value::String(output);
                    grant.payload = body.to_string().into_bytes();
                }
                let _ = writer
                    .write_all(format!("{}\n", serde_json::to_string(&grant)?).as_bytes())
                    .await;
                println!("⬡ RES : compute accordé à {}", env.from);
                continue;
            }
        }

        let known_peers = HashMap::new();
        match process_line(&line, identity, &known_peers, &mut sessions)? {
            Some((session, Received::Message(text))) => {
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("⬡ message reçu (session {session} · 4/7 fragments)");
                println!();
                println!("{text}");
                println!();
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            }
            Some((session, Received::File { name, bytes })) => {
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
        output: &crate::msg::SendOutput,
        file_name: Option<&str>,
    ) -> String {
        let from = node_id(sender);
        let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
        let sig = sender.sign_signer().unwrap().sign(&canonical_bytes(
            session,
            &from,
            to,
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
        };
        serde_json::to_string(&env).unwrap()
    }

    fn frag_json(session: &str, to: &str, idx: u8, payload: &[u8]) -> String {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: "alice".into(),
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
        };
        serde_json::to_string(&env).unwrap()
    }

    #[test]
    fn canonical_bytes_are_deterministic() {
        let a = canonical_bytes("s1", "alice", "bob", &[1, 2, 3], &[4, 5]);
        let b = canonical_bytes("s1", "alice", "bob", &[1, 2, 3], &[4, 5]);
        assert_eq!(a, b);
        let c = canonical_bytes("s1", "alice", "bob", &[1, 2, 3], &[4, 6]);
        assert_ne!(a, c, "different ciphertext must change the bytes");
    }

    #[test]
    fn full_pipeline_signed_message_round_trip() {
        let alice = LocalIdentity::generate();
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message réseau test", &pk).unwrap();
        let session = "test-session-1";
        let bob_node = node_id(&bob);
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        let kem_line = signed_kem_json(session, &alice, &bob_node, &output, None);
        let r = process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();
        assert!(r.is_none(), "kem alone must not complete a session");

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        // KEM envelope: from = Alice's node, sig+signer = Eve's key.
        let ciphertext = output.ciphertext.clone().expect("sender ciphertext");
        let sig = eve.sign_signer().unwrap().sign(&canonical_bytes(
            session,
            &alice_node,
            &bob_node,
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
        };
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();

        for frag in output.fragments.iter() {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        let kem_line = signed_kem_json(session, &alice, &bob_node, &output, None);
        process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, &output, None);
        process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();

        // Corrupt ONE fragment (flip a byte). Send exactly 4 fragments,
        // one of them corrupt: the only 4-subset reconstructs a wrong
        // secret, so the signature can never verify.
        let mut frags: Vec<_> = output.fragments.clone();
        frags[0].share[0] ^= 0xFF;
        for frag in frags.iter().take(4) {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        // Envelope addressed to carol, delivered to bob.
        let kem_line = signed_kem_json(session, &alice, &node_id(&carol), &output, None);
        let r = process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();
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
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, &output, None);
        process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();

        // Send idx=1 twice + two other distinct fragments → 3 distinct
        // fragments buffered (the duplicate is dropped), session must NOT
        // complete yet.
        let f1 = &output.fragments[0];
        let f2 = &output.fragments[1];
        let f3 = &output.fragments[2];
        let f4 = &output.fragments[3];
        process_line(
            &frag_json(session, &bob_node, f1.index, &f1.share),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        let r = process_line(
            &frag_json(session, &bob_node, f1.index, &f1.share),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        assert!(r.is_none(), "duplicate idx must be dropped");
        process_line(
            &frag_json(session, &bob_node, f2.index, &f2.share),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        process_line(
            &frag_json(session, &bob_node, f3.index, &f3.share),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        // 3 distinct fragments → nothing completed yet, duplicate not buffered.
        let buf = sessions.get(session).expect("session buffered");
        assert_eq!(buf.fragments.len(), 3, "the duplicate must not be buffered");
        // The 4th distinct fragment completes the session — normally.
        let line = frag_json(session, &bob_node, f4.index, &f4.share);
        let (_, recv) = process_line(&line, &bob, &known_peers, &mut sessions)
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
        let known_peers: HashMap<String, String> = HashMap::new();
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
        };
        process_line(
            &serde_json::to_string(&env).unwrap(),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter() {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = signed_kem_json(session, &alice, &bob_node, &output, Some("plan.txt"));
        process_line(&kem_line, &bob, &known_peers, &mut sessions).unwrap();

        // The relay must never see the plaintext name.
        let env: NetEnvelope = serde_json::from_str(&kem_line).unwrap();
        let ct = env.name_ct.expect("name_ct present");
        assert!(
            !ct.windows(b"plan.txt".len()).any(|w| w == b"plan.txt"),
            "file name must be opaque on the wire"
        );

        for (i, frag) in output.fragments.iter().enumerate() {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            let r = process_line(&line, &bob, &known_peers, &mut sessions).unwrap();
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
        let known_peers: HashMap<String, String> = HashMap::new();

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        process_line(
            &signed_kem_json(session, &alice, &bob_node, &output, None),
            &bob,
            &known_peers,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter().take(3) {
            let line = frag_json(session, &bob_node, frag.index, &frag.share);
            assert!(process_line(&line, &bob, &known_peers, &mut sessions)
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
}
