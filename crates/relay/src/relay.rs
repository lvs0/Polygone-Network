//! The relay core — async TCP server that forwards envelopes blindly.

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

/// In-memory map of connected peers (peer_id → stream).
/// NOTE: the relay doesn't *route* to these peers yet — it just keeps the
/// connection table alive. Real routing goes through libp2p relay circuit.
/// This map is only useful for the relay's liveness check.
type PeerTable = Arc<RwLock<HashMap<String, TcpStream>>>;

/// Handle one client connection.
async fn handle_client(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    _peers: PeerTable,
) -> Result<()> {
    let mut buf = [0u8; 8192];

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            // Client disconnected cleanly
            log::debug!("relay: client {} disconnected", peer_addr);
            break;
        }

        // We receive a raw JSON envelope. We do NOT parse the content.
        // We only check that it's valid JSON and relay-visible.
        let raw = &buf[..n];

        // Quick serde check — can we parse it as a JSON value?
        // This is the *only* inspection the relay does on the wire.
        let json_val: serde_json::Value = match serde_json::from_slice(raw) {
            Ok(v) => v,
            Err(_) => {
                // Not valid JSON — relay ignores it silently
                log::warn!("relay: ignoring non-JSON from {}", peer_addr);
                continue;
            }
        };

        // Extract the "kind" field to check relay-visibility.
        // If it's not a Fragment, relay does NOT forward it.
        let kind = json_val.get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match kind {
            "fragment" => {
                // This is a relay-visible envelope. Forward it.
                // In the real libp2p implementation this goes through
                // the relay circuit. In this v2 stub, we just echo it
                // back to show the plumbing works.
                log::debug!("relay: forwarding fragment from {}", peer_addr);

                // Echo to sender so they know relay received it
                stream.write_all(raw).await?;
            }
            _ => {
                // Non-relay-visible envelope (handshake, dissolve, ack).
                // Relay does not forward — these go peer-to-peer via libp2p.
                log::trace!("relay: ignored {} envelope from {}", kind, peer_addr);
            }
        }
    }
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