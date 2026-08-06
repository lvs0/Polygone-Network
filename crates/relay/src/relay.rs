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
async fn handle_client(
    stream: TcpStream,
    peer_addr: SocketAddr,
    peers: PeerTable,
) -> Result<()> {
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

    #[tokio::test]
    async fn test_relay_starts() {
        // Smoke test: does the relay bind a port? We use port 0 so the OS picks
        // a free port. The timeout tells us it started (run() loops forever).
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            run(0),
        ).await;
        // Err(TimeoutElapsed) because run() loops forever — that's fine.
        assert!(result.is_err());
    }
}
