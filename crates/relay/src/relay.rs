//! The relay core — async TCP server that forwards fragments blindly.
//!
//! Protocol (newline-delimited JSON):
//!
//! ```text
//!   client → relay : HELLO <node_id>
//!   client → relay : {"kind":"fragment","from":...,"to":...,"session":...,
//!                     "seq":...,"payload":[...]}\n
//!   relay   → peer  : (the same fragment line, forwarded verbatim)
//! ```
//!
//! The relay only reads three fields: `kind` (must be "fragment"), `to`
//! (routing), and `session` (bookkeeping). It never inspects the payload.
//! It never stores fragments. It forgets a peer the moment it disconnects.
//!
//! Hardening (Phase 1, 2026-08-07):
//! - **`from` must equal the HELLO identity** — a connection cannot emit
//!   envelopes under another node's name (anti-spoofing at routing level).
//! - **Line size cap** (64 KiB) and a **per-connection rate limit** —
//!   a misbehaving client cannot exhaust relay memory or CPU.
//! - **Sharded routing table** (16 shards) — forwarding is no longer
//!   serialized behind a single global lock.

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// Max length of a single wire line (envelope). 64 KiB bounds memory.
const MAX_LINE: usize = 64 * 1024;
/// Max envelopes forwarded per second per connection.
const RATE_LIMIT_PER_SEC: usize = 200;
/// Number of routing-table shards (parallelism for forwarding).
const SHARDS: usize = 16;

/// In-memory routing table: node_id → open write half, sharded to avoid a
/// global write lock serializing every forward.
type Shard = RwLock<HashMap<String, OwnedWriteHalf>>;
type Peers = Arc<Vec<Shard>>;

fn shard_for<'a>(peers: &'a Peers, node_id: &str) -> &'a Shard {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node_id.hash(&mut hasher);
    &peers[hasher.finish() as usize % SHARDS]
}

/// Read one line (up to `cap` bytes) without unbounded allocation.
/// Returns `None` on EOF, `Some(None)` for an oversized line.
async fn read_line_capped(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    cap: usize,
) -> std::io::Result<Option<Option<Vec<u8>>>> {
    let mut buf = Vec::with_capacity(256);
    loop {
        let mut byte = [0u8; 1];
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return Ok(if buf.is_empty() {
                None
            } else {
                Some(Some(buf))
            });
        }
        if byte[0] == b'\n' {
            return Ok(Some(Some(buf)));
        }
        buf.push(byte[0]);
        if buf.len() > cap {
            // Drain until newline so the stream stays aligned, then report.
            loop {
                let n = reader.read(&mut byte).await?;
                if n == 0 || byte[0] == b'\n' {
                    break;
                }
            }
            return Ok(Some(None));
        }
    }
}

/// Handle one client connection.
async fn handle_client(stream: TcpStream, peer_addr: SocketAddr, peers: Peers) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // ── Handshake: first line must be "HELLO <node_id>" ───────────────────
    let hello = loop {
        let line = match read_line_capped(&mut reader, MAX_LINE).await? {
            Some(Some(b)) => String::from_utf8_lossy(&b).to_string(),
            Some(None) => {
                log::warn!("relay: oversized HELLO from {} — dropping", peer_addr);
                return Ok(());
            }
            None => return Ok(()), // disconnected before HELLO
        };
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("HELLO ") {
            break rest.trim().to_string();
        }
        log::debug!("relay: ignoring non-HELLO from {}", peer_addr);
    };

    // Register the write half so fragments can be routed here.
    shard_for(&peers, &hello)
        .write()
        .await
        .insert(hello.clone(), writer);
    log::debug!("relay: {} registered as '{}'", peer_addr, hello);
    let mut dead_streams = Vec::new();

    // ── Relay loop ────────────────────────────────────────────────────────
    let mut rate_tokens = RATE_LIMIT_PER_SEC;
    let mut rate_window_start = std::time::Instant::now();
    loop {
        let line = match read_line_capped(&mut reader, MAX_LINE).await? {
            Some(Some(b)) => b,
            Some(None) => {
                log::warn!("relay: oversized line from {} — dropping", peer_addr);
                continue;
            }
            None => break, // client disconnected
        };
        if line.is_empty() {
            continue;
        }
        // Per-connection rate limit: token bucket, refilled each second.
        let now = std::time::Instant::now();
        if now.duration_since(rate_window_start).as_secs() >= 1 {
            rate_window_start = now;
            rate_tokens = RATE_LIMIT_PER_SEC;
        }
        if rate_tokens == 0 {
            log::warn!("relay: rate limit exceeded by {} — dropping", peer_addr);
            return Ok(());
        }
        rate_tokens -= 1;

        // The ONLY inspection: kind + routing fields. Never the payload.
        let val: serde_json::Value = match serde_json::from_slice(&line) {
            Ok(v) => v,
            Err(_) => {
                log::warn!("relay: ignoring non-JSON from {}", peer_addr);
                continue;
            }
        };

        let kind = val.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "fragment" {
            log::trace!("relay: ignored {} envelope from {}", kind, peer_addr);
            continue;
        }

        // Anti-spoofing: a connection forwards only under its own name.
        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("");
        if from != hello {
            log::warn!(
                "relay: '{}' tried to forward as '{}' — dropped",
                hello,
                from
            );
            continue;
        }

        let to = val.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if to.is_empty() {
            continue;
        }

        // Forward to the destination if it is connected.
        let mut table = shard_for(&peers, to).write().await;
        let mut out = line.clone();
        out.push(b'\n'); // NDJSON framing: the peer's read_line needs the newline
        let forwarded = match table.get_mut(to) {
            Some(dst) => {
                // Dead streams are dropped on write failure.
                let mut ok = true;
                if let Err(e) = dst.write_all(&out).await {
                    log::warn!("relay: drop dead peer '{}': {}", to, e);
                    ok = false;
                    dead_streams.push(to.to_string());
                }
                ok
            }
            None => {
                log::debug!("relay: '{}' not connected — fragment dropped", to);
                false
            }
        };

        for dead in dead_streams.drain(..) {
            table.remove(&dead);
        }

        if forwarded {
            log::debug!(
                "relay: forwarded fragment (session={:?}) from '{}' to '{}'",
                val.get("session").and_then(|v| v.as_str()),
                hello,
                to
            );
        }
    }

    log::debug!("relay: '{}' ({}) disconnected", hello, peer_addr);
    shard_for(&peers, &hello).write().await.remove(&hello);
    Ok(())
}

/// Start the relay TCP server on the given port.
pub async fn run(port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("relay: listening on {}", addr);
    run_listener(listener).await
}

/// Serve on an already-bound listener (used by tests with port 0).
async fn run_listener(listener: TcpListener) -> Result<()> {
    let mut shards = Vec::with_capacity(SHARDS);
    for _ in 0..SHARDS {
        shards.push(RwLock::new(HashMap::new()));
    }
    let peers: Peers = Arc::new(shards);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let peers = peers.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, peer_addr, peers).await {
                        log::error!("relay: client error {}: {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                log::error!("relay: accept error: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::AsyncBufReadExt;

    /// Spawn a relay on a random port, return the bound address.
    async fn spawn_relay() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = run_listener(listener).await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        addr
    }

    #[tokio::test]
    async fn test_relay_starts() {
        // Smoke test: does the relay bind a port? We use port 0 so the OS picks
        // a free port. The timeout tells us it started (run() loops forever).
        let result = tokio::time::timeout(Duration::from_millis(200), run(0)).await;
        // Err(TimeoutElapsed) because run() loops forever — that's fine.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn routes_fragments_to_registered_peer() {
        let addr = spawn_relay().await;

        // Bob registers.
        let mut bob = TcpStream::connect(addr).await.unwrap();
        bob.write_all(b"HELLO bob\n").await.unwrap();

        // Alice registers.
        let mut alice = TcpStream::connect(addr).await.unwrap();
        alice.write_all(b"HELLO alice\n").await.unwrap();

        // Let registration settle.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let env = r#"{"kind":"fragment","from":"alice","to":"bob","session":"s1","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[1,2,3]}"#;
        alice
            .write_all(format!("{env}\n").as_bytes())
            .await
            .unwrap();

        // Bob receives the fragment, forwarded verbatim.
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut line).await.unwrap();
        })
        .await
        .unwrap();
        assert!(line.contains("\"session\":\"s1\""), "forwarded: {line}");
        assert!(
            line.contains("\"payload\":[1,2,3]"),
            "forwarded verbatim: {line}"
        );
    }

    #[tokio::test]
    async fn drops_fragments_for_offline_peer_without_error() {
        let addr = spawn_relay().await;

        let mut alice = TcpStream::connect(addr).await.unwrap();
        alice.write_all(b"HELLO alice\n").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Nobody registered as "ghost" — the relay must drop, not crash.
        let env = r#"{"kind":"fragment","from":"alice","to":"ghost","session":"s9","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[9,9]}"#;
        alice
            .write_all(format!("{env}\n").as_bytes())
            .await
            .unwrap();

        // Alice's connection must still be usable after the drop.
        let env2 = r#"{"kind":"fragment","from":"alice","to":"ghost","session":"s10","seq":2,"type":"frag","idx":2,"threshold":4,"total":7,"payload":[8,8]}"#;
        alice
            .write_all(format!("{env2}\n").as_bytes())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn ignores_non_fragment_envelopes() {
        let addr = spawn_relay().await;

        let mut bob = TcpStream::connect(addr).await.unwrap();
        bob.write_all(b"HELLO bob\n").await.unwrap();
        let mut alice = TcpStream::connect(addr).await.unwrap();
        alice.write_all(b"HELLO alice\n").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Handshake/dissolve envelopes are NOT forwarded by the relay.
        let handshake = r#"{"kind":"handshake_init","from":"alice","to":"bob","session":null,"seq":0,"payload":[]}"#;
        alice
            .write_all(format!("{handshake}\n").as_bytes())
            .await
            .unwrap();

        // Bob must NOT receive it (read with a short timeout → timeout expected).
        let mut line = String::new();
        let got = tokio::time::timeout(Duration::from_millis(300), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut line).await.unwrap();
        })
        .await;
        assert!(got.is_err(), "relay must not forward non-fragments");
    }

    #[tokio::test]
    async fn drops_envelopes_with_mismatched_from() {
        let addr = spawn_relay().await;

        let mut bob = TcpStream::connect(addr).await.unwrap();
        bob.write_all(b"HELLO bob\n").await.unwrap();
        let mut alice = TcpStream::connect(addr).await.unwrap();
        alice.write_all(b"HELLO alice\n").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Alice tries to forward under Mallory's name — anti-spoofing.
        let forged = r#"{"kind":"fragment","from":"mallory","to":"bob","session":"sX","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[9,9]}"#;
        alice
            .write_all(format!("{forged}\n").as_bytes())
            .await
            .unwrap();

        let mut line = String::new();
        let got = tokio::time::timeout(Duration::from_millis(300), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut line).await.unwrap();
        })
        .await;
        assert!(got.is_err(), "relay must not forward a mismatched from");
    }

    #[tokio::test]
    async fn oversized_lines_are_dropped_not_forwarded() {
        let addr = spawn_relay().await;

        let mut bob = TcpStream::connect(addr).await.unwrap();
        bob.write_all(b"HELLO bob\n").await.unwrap();
        let mut alice = TcpStream::connect(addr).await.unwrap();
        alice.write_all(b"HELLO alice\n").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // One line bigger than MAX_LINE, then a valid fragment.
        let huge = "x".repeat(MAX_LINE + 100);
        alice.write_all(huge.as_bytes()).await.unwrap();
        alice.write_all(b"\n").await.unwrap();
        let ok = r#"{"kind":"fragment","from":"alice","to":"bob","session":"sY","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[1,2,3]}"#;
        alice.write_all(format!("{ok}\n").as_bytes()).await.unwrap();

        // Bob receives ONLY the valid fragment.
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut line).await.unwrap();
        })
        .await
        .unwrap();
        assert!(line.contains("\"session\":\"sY\""), "forwarded: {line}");
        assert!(
            !line.contains("xxxx") && !line.contains("sX"),
            "oversized lines must never be forwarded: {line}"
        );
    }
}
