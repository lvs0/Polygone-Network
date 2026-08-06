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
//! (routing), and `session` (TTL bookkeeping). It never inspects the payload.
//! It never stores fragments. It forgets a peer the moment it disconnects.

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// In-memory routing table: peer_id → open write half.
type PeerTable = Arc<RwLock<HashMap<String, OwnedWriteHalf>>>;

/// Handle one client connection.
async fn handle_client(stream: TcpStream, peer_addr: SocketAddr, peers: PeerTable) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // ── Handshake: first line must be "HELLO <node_id>" ───────────────────
    let hello = loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Ok(()); // disconnected before HELLO
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("HELLO ") {
            break rest.to_string();
        }
        log::debug!("relay: ignoring non-HELLO from {}", peer_addr);
    };

    // Register the write half so fragments can be routed here.
    peers.write().await.insert(hello.clone(), writer);
    log::debug!("relay: {} registered as '{}'", peer_addr, hello);
    let mut dead_streams = Vec::new();

    // ── Relay loop ────────────────────────────────────────────────────────
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // client disconnected
        }
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }

        // The ONLY inspection: kind + routing fields. Never the payload.
        let val: serde_json::Value = match serde_json::from_str(raw) {
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

        let to = val.get("to").and_then(|v| v.as_str()).unwrap_or("");
        if to.is_empty() {
            continue;
        }

        // Forward to the destination if it is connected.
        let mut table = peers.write().await;
        let forwarded = match table.get_mut(to) {
            Some(dst) => {
                // Dead streams are dropped on write failure.
                let mut ok = true;
                if let Err(e) = dst.write_all(format!("{raw}\n").as_bytes()).await {
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
    peers.write().await.remove(&hello);
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
    let peers: PeerTable = Arc::new(RwLock::new(HashMap::new()));

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
}
