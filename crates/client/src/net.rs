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

/// A receiver-side session buffer.
#[derive(Default)]
struct SessionBuffer {
    kem_ct: Option<kem::KemCiphertext>,
    name: Option<String>,
    fragments: Vec<shamir::Fragment>,
}

/// Listen for incoming messages through the relay. Blocks until interrupted.
/// Reconstructs and decrypts each session as soon as 4/7 fragments arrive.
/// File transfers are saved to `~/.polygone/received/`.
pub async fn receive_network(relay: &str, identity: &LocalIdentity) -> Result<()> {
    let stream = connect(relay, identity).await?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut sessions: HashMap<String, SessionBuffer> = HashMap::new();

    println!(
        "⬡ en écoute via relay {} — node {}",
        relay,
        node_id(identity)
    );
    println!("  (Ctrl-C pour arrêter)\n");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            println!("relay déconnecté.");
            return Ok(());
        }
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }
        let env: NetEnvelope = match serde_json::from_str(raw) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if env.kind != "fragment" {
            continue;
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

        // Enough fragments to attempt reconstruction?
        if buf.kem_ct.is_some() && buf.fragments.len() >= 4 {
            let kem_ct = buf.kem_ct.clone().expect("checked");
            let shared = kem::decapsulate(&identity.kem_secret_key()?, &kem_ct)?;
            let key = symmetric::SessionKey::derive_from_secret(&shared);
            let ciphertext = shamir::reconstruct(&buf.fragments, 4)?;
            match symmetric::decrypt(&ciphertext, &key) {
                Ok(plain) => match buf.name.clone() {
                    Some(name) => {
                        let dir = received_dir()?;
                        let path = dir.join(sanitize_name(&name));
                        std::fs::write(&path, &plain)?;
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!(
                            "⬡ fichier reçu : {} ({} octets · session {} · {} fragments)",
                            path.display(),
                            plain.len(),
                            env.session,
                            buf.fragments.len()
                        );
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        sessions.remove(&env.session);
                    }
                    None => {
                        let text = String::from_utf8_lossy(&plain).to_string();
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!(
                            "⬡ message reçu (session {} · {} fragments)",
                            env.session,
                            buf.fragments.len()
                        );
                        println!();
                        println!("{text}");
                        println!();
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        sessions.remove(&env.session);
                    }
                },
                Err(e) => {
                    println!("⚠ reconstruction échouée : {e}");
                    sessions.remove(&env.session);
                }
            }
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
