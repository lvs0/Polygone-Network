//! Client-side logic for Polygone v2.

use anyhow::Result;
use polygone_core::{Envelope, Fragment, NodeId, SessionId, FRAGMENT_SHARES};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Send a message to the relay.
///
/// In the v2 stub, the message is:
/// 1. Wrapped in a dummy Fragment (real: encrypted + Shamir-split)
/// 2. Serialised to JSON as a relay-visible Envelope
/// 3. Written to the relay TCP socket
///
/// The relay echoes it back to confirm receipt.
pub async fn send_msg(msg: &str) -> Result<()> {
    let from = NodeId::random();
    let to = NodeId::random(); // receiver's NodeId (real: shared via libp2p)

    // Build a dummy fragment — real version: encrypt(msg), then Shamir-split
    let session = SessionId::random();
    let frag = Fragment {
        session_id: session,
        index: 0,
        threshold: 4,
        total: FRAGMENT_SHARES as u8,
        content_hash: [0u8; 32],
        payload: msg.as_bytes().to_vec(),
    };

    let envelope = Envelope::from_fragment(from, to, &frag);
    let json = serde_json::to_string(&envelope)?;

    log::info!("sending {} bytes to relay (session={})", json.len(), session);

    // Connect to relay
    let mut stream = TcpStream::connect("127.0.0.1:7000").await?;
    stream.write_all(json.as_bytes()).await?;

    // Read relay echo
    let mut reply = vec![0u8; 8192];
    let n = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut reply)).await??;
    let response = String::from_utf8_lossy(&reply[..n]);
    log::info!("relay echoed {} bytes: {}", n, &response[..response.len().min(120)]);

    Ok(())
}

/// Wait for incoming messages (Charlie in the demo).
pub async fn receive() -> Result<()> {
    log::info!("client: listen mode not yet implemented (libp2p circuit needed)");
    log::info!("client: the real path is the in-process `polygone demo` (crates/client/src/demo.rs)");
    Ok(())
}
