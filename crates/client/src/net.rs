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
    /// Present only on the KEM envelope of a file transfer.
    #[serde(default)]
    name: Option<String>,
}

/// The node id of an identity: first 16 hex chars of the KEM public key.
pub fn node_id(identity: &LocalIdentity) -> String {
    identity.kem_pk_hex.chars().take(16).collect()
}

/// Random session id (hex).
fn session_id() -> String {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut b = [0u8; 8];
    rng.fill_bytes(&mut b);
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

    let mut stream = connect(relay, identity).await?;

    // 1. The KEM ciphertext envelope (carries the file name, if any).
    let kem_env = NetEnvelope {
        kind: "fragment".into(),
        from: from.clone(),
        to: dest_node.to_string(),
        session: session.clone(),
        seq: 0,
        typ: "kem".into(),
        idx: 0,
        threshold: 4,
        total: 7,
        payload: output.kem_ct.as_bytes().to_vec(),
        name: name.map(|s| s.to_string()),
    };
    stream
        .write_all(format!("{}\n", serde_json::to_string(&kem_env)?).as_bytes())
        .await?;

    // 2. The 7 fragment envelopes.
    for (i, frag) in output.fragments.iter().enumerate() {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: from.clone(),
            to: dest_node.to_string(),
            session: session.clone(),
            seq: i as u64 + 1,
            typ: "frag".into(),
            idx: frag.index,
            threshold: 4,
            total: 7,
            payload: frag.share.clone(),
            name: None,
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
        payload: serde_json::json!({
            "action": "compute",
            "task": task,
        })
        .to_string()
        .into_bytes(),
        name: None,
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
    kem_ct: Option<kem::KemCiphertext>,
    name: Option<String>,
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
        name: None,
    }
}

/// Run the task from a RES request inside the systemd sandbox and return
/// the output. Empty input = no execution (grant without output).
fn run_res_task(req: &NetEnvelope) -> Option<String> {
    let body: serde_json::Value = serde_json::from_slice(&req.payload).ok()?;
    let task = body.get("task")?.as_str()?;
    if task.trim().is_empty() {
        return None;
    }
    match crate::exec::run_sandboxed(task, 256, 50, std::time::Duration::from_secs(30)) {
        Ok(out) => Some(out),
        Err(e) => Some(format!("[erreur sandbox] {e}")),
    }
}

/// Process one wire line against the session map. Returns `Some` when a
/// session completes (>= 4/7 fragments) and decrypts. Pure logic — no sockets.
fn process_line(
    line: &str,
    identity: &LocalIdentity,
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

    let buf = sessions.entry(env.session.clone()).or_default();
    match env.typ.as_str() {
        "kem" => match kem::KemCiphertext::from_bytes(&env.payload) {
            Ok(ct) => {
                buf.kem_ct = Some(ct);
                buf.name = env.name.clone();
            }
            Err(_) => {}
        },
        "frag" => {
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

    let kem_ct = buf.kem_ct.clone().expect("checked");
    let shared = kem::decapsulate(&identity.kem_secret_key()?, &kem_ct)?;
    let key = symmetric::SessionKey::derive_from_secret(&shared);
    let ciphertext = shamir::reconstruct(&buf.fragments, 4)?;
    let plain = symmetric::decrypt(&ciphertext, &key)?;

    let session = env.session.clone();
    let received = match buf.name.clone() {
        Some(name) => Received::File { name, bytes: plain },
        None => Received::Message(String::from_utf8_lossy(&plain).to_string()),
    };
    sessions.remove(&session);
    Ok(Some((session, received)))
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
        if let Ok(env) = serde_json::from_str::<NetEnvelope>(line.trim()) {
            if env.typ == "req" && compute {
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

        match process_line(&line, identity, &mut sessions)? {
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

    fn envelope_json(
        session: &str,
        typ: &str,
        idx: u8,
        payload: &[u8],
        name: Option<&str>,
    ) -> String {
        let env = NetEnvelope {
            kind: "fragment".into(),
            from: "alice".into(),
            to: "bob".into(),
            session: session.into(),
            seq: 0,
            typ: typ.into(),
            idx,
            threshold: 4,
            total: 7,
            payload: payload.to_vec(),
            name: name.map(|s| s.to_string()),
        };
        serde_json::to_string(&env).unwrap()
    }

    #[test]
    fn full_pipeline_message_round_trip() {
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("message réseau test", &pk).unwrap();
        let session = "test-session-1";

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

        // KEM envelope first, then only 4 of the 7 fragments.
        let kem_line = envelope_json(session, "kem", 0, output.kem_ct.as_bytes(), None);
        let r = process_line(&kem_line, &bob, &mut sessions).unwrap();
        assert!(r.is_none(), "kem alone must not complete a session");

        for frag in output.fragments.iter().take(4) {
            let line = envelope_json(session, "frag", frag.index, &frag.share, None);
            let r = process_line(&line, &bob, &mut sessions).unwrap();
            if frag.index == 4 {
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
    fn full_pipeline_file_round_trip() {
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let bytes = b"contenu secret du fichier".to_vec();
        let output = crate::msg::send_bytes(&bytes, &pk).unwrap();
        let session = "test-session-file";

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        let kem_line = envelope_json(
            session,
            "kem",
            0,
            output.kem_ct.as_bytes(),
            Some("plan.txt"),
        );
        process_line(&kem_line, &bob, &mut sessions).unwrap();

        for frag in output.fragments.iter().take(4) {
            let line = envelope_json(session, "frag", frag.index, &frag.share, None);
            let r = process_line(&line, &bob, &mut sessions).unwrap();
            if let Some((_, Received::File { name, bytes: got })) = r {
                assert_eq!(name, "plan.txt");
                assert_eq!(got, bytes);
                return;
            }
        }
        panic!("file session never completed");
    }

    #[test]
    fn three_fragments_never_complete() {
        let bob = LocalIdentity::generate();
        let pk = bob.kem_public_key().unwrap();
        let output = crate::msg::send("trop peu de fragments", &pk).unwrap();
        let session = "test-incomplete";

        let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();
        process_line(
            &envelope_json(session, "kem", 0, output.kem_ct.as_bytes(), None),
            &bob,
            &mut sessions,
        )
        .unwrap();
        for frag in output.fragments.iter().take(3) {
            let line = envelope_json(session, "frag", frag.index, &frag.share, None);
            assert!(process_line(&line, &bob, &mut sessions).unwrap().is_none());
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
            name: None,
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
