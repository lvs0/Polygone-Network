//! The relay core — async TCP server that forwards fragments blindly.
//!
//! Protocol (newline-delimited JSON):
//!
//! ```text
//!   client → relay : HELLO <node_id>
//!   relay   → peer  : HELLO_OK\n        (registration accepted — the slot is yours)
//!   relay   → peer  : HELLO_DENIED\n    (another connection already owns this node_id)
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
//! - **Registration is acknowledged** (`HELLO_OK` / `HELLO_DENIED`) — a
//!   duplicate `HELLO` for a live node_id is refused and the claimant is
//!   told, so routing can never be silently stolen (no last-writer-wins).
//! - **Line size cap** (64 KiB) and a **per-connection rate limit** —
//!   a misbehaving client cannot exhaust relay memory or CPU.
//! - **Sharded routing table** (16 shards) — forwarding is no longer
//!   serialized behind a single global lock.

use anyhow::Result;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

/// Max length of a single wire line (envelope). 64 KiB bounds memory.
const MAX_LINE: usize = 64 * 1024;
/// Max envelopes forwarded per second per connection.
const RATE_LIMIT_PER_SEC: usize = 200;
/// Max simultaneous connections (anti-DoS): each holds a routing entry.
const MAX_CONNECTIONS: usize = 1024;
/// A TCP client must complete HELLO promptly or release its connection slot.
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Once a frame starts, it must finish promptly; idle registered sockets remain open.
const FRAME_CONTINUATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// A slow recipient must not pin a relay task and a per-peer writer forever.
const DESTINATION_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Bound attacker-controlled routing keys even before registration.
const MAX_NODE_ID_LEN: usize = 64;
/// Number of routing-table shards (parallelism for forwarding).
const SHARDS: usize = 16;

/// In-memory routing table: node_id → open write half, sharded to avoid a
/// global write lock serializing every forward.
type PeerWriter = Arc<Mutex<OwnedWriteHalf>>;
type Shard = RwLock<HashMap<String, PeerWriter>>;
type Peers = Arc<Vec<Shard>>;

/// Remove a route only if it still points to the connection that observed the
/// failure/disconnect. A stale handler must never erase a replacement peer.
fn remove_peer_if_current(
    table: &mut HashMap<String, PeerWriter>,
    node_id: &str,
    expected: &PeerWriter,
) -> bool {
    if table
        .get(node_id)
        .is_some_and(|current| Arc::ptr_eq(current, expected))
    {
        table.remove(node_id);
        true
    } else {
        false
    }
}

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
    continuation_timeout: std::time::Duration,
) -> std::io::Result<Option<Option<Vec<u8>>>> {
    let mut buf = Vec::with_capacity(256);
    let mut deadline = None;
    loop {
        let available = match deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, reader.fill_buf())
                .await
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "relay frame continuation timed out",
                    )
                })??,
            None => reader.fill_buf().await?,
        };
        if available.is_empty() {
            return Ok(if buf.is_empty() {
                None
            } else {
                Some(Some(buf))
            });
        }

        if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
            if buf.len().saturating_add(newline) > cap {
                reader.consume(newline + 1);
                return Ok(Some(None));
            }
            buf.extend_from_slice(&available[..newline]);
            reader.consume(newline + 1);
            return Ok(Some(Some(buf)));
        }

        let available_len = available.len();
        if buf.len().saturating_add(available_len) > cap {
            let drain_deadline =
                deadline.unwrap_or_else(|| tokio::time::Instant::now() + continuation_timeout);
            reader.consume(available_len);
            // Drain only this oversized line, with the same absolute deadline.
            loop {
                let chunk = tokio::time::timeout_at(drain_deadline, reader.fill_buf())
                    .await
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "oversized relay frame continuation timed out",
                        )
                    })??;
                if chunk.is_empty() {
                    return Ok(Some(None));
                }
                if let Some(newline) = chunk.iter().position(|byte| *byte == b'\n') {
                    reader.consume(newline + 1);
                    return Ok(Some(None));
                }
                let len = chunk.len();
                reader.consume(len);
            }
        }

        buf.extend_from_slice(available);
        reader.consume(available_len);
        if deadline.is_none() {
            deadline = Some(tokio::time::Instant::now() + continuation_timeout);
        }
    }
}

/// Atomically reserve a connection slot. A load followed by fetch_add is
/// racy: simultaneous accepts could all pass the cap before incrementing.
fn try_acquire_connection_slot(conns: &std::sync::atomic::AtomicUsize, max: usize) -> bool {
    use std::sync::atomic::Ordering;
    let mut current = conns.load(Ordering::Relaxed);
    loop {
        if current >= max {
            return false;
        }
        match conns.compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Relaxed)
        {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

fn valid_node_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_NODE_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Read and validate a HELLO under a bounded deadline. Non-HELLO lines are
/// tolerated for compatibility, but cannot keep an unauthenticated socket
/// alive indefinitely.
async fn read_hello(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    timeout: std::time::Duration,
) -> Result<Option<String>> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let result = match tokio::time::timeout_at(
            deadline,
            read_line_capped(reader, MAX_LINE, FRAME_CONTINUATION_TIMEOUT),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => return Ok(None),
        };
        let line = match result {
            Some(Some(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
            Some(None) => anyhow::bail!("oversized HELLO"),
            None => return Ok(None),
        };
        if let Some(rest) = line.trim().strip_prefix("HELLO ") {
            let node_id = rest.trim();
            if !valid_node_id(node_id) {
                anyhow::bail!("invalid node_id in HELLO");
            }
            return Ok(Some(node_id.to_string()));
        }
    }
}

async fn write_frame_with_timeout<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &[u8],
    timeout: std::time::Duration,
) -> io::Result<()> {
    match tokio::time::timeout(timeout, writer.write_all(frame)).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "relay destination write timed out",
        )),
    }
}

fn outbound_line(line: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len() + 1);
    out.extend_from_slice(line);
    out.push(b'\n');
    out
}

/// Handle one client connection.
async fn handle_client(stream: TcpStream, peer_addr: SocketAddr, peers: Peers) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // ── Handshake: bounded, validated, and required before registration ───
    let hello = match read_hello(&mut reader, HANDSHAKE_TIMEOUT).await {
        Ok(Some(node_id)) => node_id,
        Ok(None) => return Ok(()),
        Err(error) => {
            log::warn!("relay: invalid HELLO from {}: {}", peer_addr, error);
            return Ok(());
        }
    };

    // ── Registration: the slot, decided and acknowledged ───────────────────
    // A node_id already connected is NOT overwritten: an attacker cannot
    // steal the incoming slot of a live peer. The ack makes the outcome
    // deterministic — a client knows, before sending a single byte, whether
    // it owns the slot. A denied registration never silently sends its
    // traffic into a slot it does not own.
    let route_writer = {
        let mut table = shard_for(&peers, &hello).write().await;
        if table.contains_key(&hello) {
            log::warn!(
                "relay: '{}' already connected — rejecting duplicate from {}",
                hello,
                peer_addr
            );
            let _ = writer.write_all(b"HELLO_DENIED\n").await;
            return Ok(());
        }
        let _ = writer.write_all(b"HELLO_OK\n").await;
        let route_writer = Arc::new(Mutex::new(writer));
        table.insert(hello.clone(), route_writer.clone());
        route_writer
    };
    log::debug!("relay: {} registered as '{}'", peer_addr, hello);
    // Keep cleanup outside the loop future so every exit path (EOF, I/O
    // error, rate limit) unregisters this exact live slot.
    let relay_result: Result<()> = async {
        let mut rate_tokens = RATE_LIMIT_PER_SEC;
        let mut rate_window_start = std::time::Instant::now();
        loop {
            let line =
                match read_line_capped(&mut reader, MAX_LINE, FRAME_CONTINUATION_TIMEOUT).await? {
                    Some(Some(bytes)) => bytes,
                    Some(None) => {
                        log::warn!("relay: oversized line from {} — dropping", peer_addr);
                        continue;
                    }
                    None => break,
                };
            if line.is_empty() {
                continue;
            }

            let now = std::time::Instant::now();
            if now.duration_since(rate_window_start).as_secs() >= 1 {
                rate_window_start = now;
                rate_tokens = RATE_LIMIT_PER_SEC;
            }
            if rate_tokens == 0 {
                log::warn!("relay: rate limit exceeded by {} — dropping", peer_addr);
                break;
            }
            rate_tokens -= 1;

            let value: serde_json::Value = match serde_json::from_slice(&line) {
                Ok(value) => value,
                Err(_) => {
                    log::warn!("relay: ignoring non-JSON from {}", peer_addr);
                    continue;
                }
            };
            let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind != "fragment" {
                log::trace!("relay: ignored {} envelope from {}", kind, peer_addr);
                continue;
            }

            let from = value.get("from").and_then(|v| v.as_str()).unwrap_or("");
            if from != hello {
                log::warn!(
                    "relay: '{}' tried to forward as '{}' — dropped",
                    hello,
                    from
                );
                continue;
            }
            let to = value.get("to").and_then(|v| v.as_str()).unwrap_or("");
            if to.is_empty() {
                continue;
            }

            // Clone the destination writer under a short routing-table read
            // lock, then serialize writes per peer. No shard lock is held over I/O.
            let destination = shard_for(&peers, to).read().await.get(to).cloned();
            let forwarded = if let Some(destination) = destination {
                let write_result = {
                    let mut writer = destination.lock().await;
                    let frame = outbound_line(&line);
                    let result =
                        write_frame_with_timeout(&mut *writer, &frame, DESTINATION_WRITE_TIMEOUT)
                            .await;
                    if result.is_err() {
                        // Close this writer before releasing its mutex. Any
                        // already-cloned routes then fail instead of reusing
                        // a socket that stalled or partially wrote a frame.
                        let _ = tokio::time::timeout(DESTINATION_WRITE_TIMEOUT, writer.shutdown())
                            .await;
                    }
                    result
                };
                match write_result {
                    Ok(()) => true,
                    Err(error) => {
                        log::warn!("relay: drop dead peer '{}': {}", to, error);
                        let mut table = shard_for(&peers, to).write().await;
                        remove_peer_if_current(&mut table, to, &destination);
                        false
                    }
                }
            } else {
                log::debug!("relay: '{}' not connected — fragment dropped", to);
                false
            };

            if forwarded {
                log::debug!(
                    "relay: forwarded fragment (session={:?}) from '{}' to '{}'",
                    value.get("session").and_then(|v| v.as_str()),
                    hello,
                    to
                );
            }
        }
        Ok(())
    }
    .await;

    log::debug!("relay: '{}' ({}) disconnected", hello, peer_addr);
    let mut table = shard_for(&peers, &hello).write().await;
    remove_peer_if_current(&mut table, &hello, &route_writer);
    relay_result
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
    // Global connection cap: an attacker cannot open unbounded connections
    // (each holds a routing entry + buffers). Beyond the cap, connections
    // are accepted and dropped immediately.
    let conns = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                if !try_acquire_connection_slot(&conns, MAX_CONNECTIONS) {
                    log::warn!(
                        "relay: connection cap ({MAX_CONNECTIONS}) reached — dropping {}",
                        peer_addr
                    );
                    drop(stream);
                    continue;
                }
                let peers = peers.clone();
                let conns = conns.clone();
                tokio::spawn(async move {
                    let _guard = ConnGuard(conns);
                    if let Err(e) = handle_client(stream, peer_addr, peers).await {
                        log::error!("relay: client error {}: {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                log::error!("relay: accept error: {}", e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

/// Releases the connection slot on drop.
struct ConnGuard(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::AsyncBufReadExt;

    #[tokio::test]
    async fn destination_write_deadline_drops_a_non_reading_peer() {
        let (mut writer, _reader) = tokio::io::duplex(8);
        let error = write_frame_with_timeout(&mut writer, &[0u8; 64], Duration::from_millis(20))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn stale_disconnect_cannot_remove_a_replacement_route() {
        async fn writer_half() -> OwnedWriteHalf {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
            let _client = client.unwrap();
            let (server, _) = accepted.unwrap();
            let (_reader, writer) = server.into_split();
            writer
        }
        let old_route = Arc::new(Mutex::new(writer_half().await));
        let new_route = Arc::new(Mutex::new(writer_half().await));
        let mut table = HashMap::new();
        table.insert("alice".to_string(), new_route.clone());

        assert!(!remove_peer_if_current(&mut table, "alice", &old_route));
        assert!(table
            .get("alice")
            .is_some_and(|route| Arc::ptr_eq(route, &new_route)));
        assert!(remove_peer_if_current(&mut table, "alice", &new_route));
        assert!(!table.contains_key("alice"));
    }

    #[test]
    fn connection_slot_cap_is_atomic_under_contention() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Barrier};
        let slots = Arc::new(AtomicUsize::new(0));
        let successes = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(32));
        let threads: Vec<_> = (0..32)
            .map(|_| {
                let slots = slots.clone();
                let successes = successes.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    if try_acquire_connection_slot(&slots, 5) {
                        successes.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(successes.load(Ordering::Relaxed), 5);
        assert_eq!(slots.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn node_ids_are_bounded_and_safe_for_routing_keys() {
        assert!(valid_node_id("0123456789abcdef"));
        assert!(valid_node_id("legacy-node_1"));
        assert!(!valid_node_id(""));
        assert!(!valid_node_id("node with spaces"));
        assert!(!valid_node_id(&"a".repeat(MAX_NODE_ID_LEN + 1)));
        assert!(!valid_node_id("node\nHELLO attacker"));
    }

    #[tokio::test]
    async fn partial_frame_times_out_after_its_first_byte() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
        let mut client = client.unwrap();
        let (server, _) = accepted.unwrap();
        let (read, _write) = server.into_split();
        let mut reader = BufReader::new(read);
        client.write_all(b"{\"kind\":").await.unwrap();
        let error = read_line_capped(&mut reader, MAX_LINE, Duration::from_millis(20))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    #[tokio::test]
    async fn incomplete_handshake_times_out_and_releases_the_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr);
        let (_client_stream, accepted) = tokio::join!(client, listener.accept());
        let (server, _) = accepted.unwrap();
        let (read, _write) = server.into_split();
        let mut reader = BufReader::new(read);
        let result = read_hello(&mut reader, Duration::from_millis(20))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn malformed_or_oversized_node_ids_are_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (client, accepted) = tokio::join!(TcpStream::connect(addr), listener.accept());
        let mut client = client.unwrap();
        let (server, _) = accepted.unwrap();
        let (read, _write) = server.into_split();
        let mut reader = BufReader::new(read);
        client.write_all(b"HELLO invalid id\n").await.unwrap();
        let result = read_hello(&mut reader, Duration::from_secs(1)).await;
        assert!(result.is_err());
    }

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

    /// Register at the relay and consume the `HELLO_OK` acknowledgment —
    /// after this returns, the connection provably owns its node_id slot.
    async fn hello(stream: &mut TcpStream, id: &str) {
        stream
            .write_all(format!("HELLO {id}\n").as_bytes())
            .await
            .unwrap();
        let mut ack = String::new();
        let mut reader = BufReader::new(&mut *stream);
        reader.read_line(&mut ack).await.unwrap();
        assert_eq!(
            ack.trim(),
            "HELLO_OK",
            "relay must accept HELLO {id}: {ack}"
        );
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
    async fn rate_limited_connection_releases_its_node_id_slot() {
        let addr = spawn_relay().await;
        let mut first = TcpStream::connect(addr).await.unwrap();
        hello(&mut first, "alice").await;
        let envelope = r#"{"kind":"fragment","from":"alice","to":"offline","session":"rate-limit","seq":1,"payload":[]}"#;
        for _ in 0..=RATE_LIMIT_PER_SEC {
            first
                .write_all(format!("{envelope}\n").as_bytes())
                .await
                .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        // If early return skipped cleanup, this same node ID would be denied.
        let mut replacement = TcpStream::connect(addr).await.unwrap();
        hello(&mut replacement, "alice").await;
    }

    #[tokio::test]
    async fn routes_fragments_to_registered_peer() {
        let addr = spawn_relay().await;

        // Bob registers (and provably owns his slot: HELLO_OK).
        let mut bob = TcpStream::connect(addr).await.unwrap();
        hello(&mut bob, "bob").await;

        // Alice registers.
        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

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
        hello(&mut alice, "alice").await;

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
        hello(&mut bob, "bob").await;
        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

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
        hello(&mut bob, "bob").await;
        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

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
    async fn fragment_is_forwarded_only_to_its_intended_recipient() {
        let addr = spawn_relay().await;

        let mut bob = TcpStream::connect(addr).await.unwrap();
        hello(&mut bob, "bob").await;
        let mut carol = TcpStream::connect(addr).await.unwrap();
        hello(&mut carol, "carol").await;
        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

        let fragment = r#"{"kind":"fragment","from":"alice","to":"bob","session":"routing-test","seq":1,"payload":[1,2,3]}"#;
        alice
            .write_all(format!("{fragment}\n").as_bytes())
            .await
            .unwrap();

        let mut bob_line = String::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut bob_line).await.unwrap();
        })
        .await
        .expect("intended recipient should receive the fragment");
        assert_eq!(bob_line.trim(), fragment);

        let mut carol_line = String::new();
        let received = tokio::time::timeout(Duration::from_millis(200), async {
            let mut reader = BufReader::new(&mut carol);
            reader.read_line(&mut carol_line).await.unwrap()
        })
        .await;
        assert!(
            received.is_err(),
            "unrelated recipient received a fragment: {carol_line}"
        );
    }

    #[tokio::test]
    async fn fragment_for_disconnected_recipient_is_dropped() {
        let addr = spawn_relay().await;

        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

        let missing = r#"{"kind":"fragment","from":"alice","to":"not-connected","session":"missing-test","seq":1,"payload":[9]}"#;
        alice
            .write_all(format!("{missing}\n").as_bytes())
            .await
            .unwrap();

        // Routing to an absent peer is a drop, not a connection failure: the
        // same sender can still forward to a subsequently connected peer.
        let mut bob = TcpStream::connect(addr).await.unwrap();
        hello(&mut bob, "bob").await;
        let valid = r#"{"kind":"fragment","from":"alice","to":"bob","session":"after-missing","seq":2,"payload":[4]}"#;
        alice
            .write_all(format!("{valid}\n").as_bytes())
            .await
            .unwrap();

        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            let mut reader = BufReader::new(&mut bob);
            reader.read_line(&mut line).await.unwrap();
        })
        .await
        .expect("sender should continue routing after a missing recipient");
        assert_eq!(line.trim(), valid);
    }

    #[tokio::test]
    async fn oversized_lines_are_dropped_not_forwarded() {
        let addr = spawn_relay().await;

        let mut bob = TcpStream::connect(addr).await.unwrap();
        hello(&mut bob, "bob").await;
        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

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

    // ── Anti-squatting : un node_id déjà connecté n'est PAS écrasé ──────────
    // La victime s'enregistre et reçoit HELLO_OK : elle POSSÈDE le slot.
    // L'attaquante tente le même node_id : le relay refuse et le DIT
    // (HELLO_DENIED) — pas de last-writer-wins, pas de vol silencieux.
    #[tokio::test]
    async fn duplicate_hello_does_not_steal_routing() {
        let addr = spawn_relay().await;

        // La vraie victime s'enregistre — l'ack prouve que son slot est pris.
        let mut victim = TcpStream::connect(addr).await.unwrap();
        victim.write_all(b"HELLO bob\n").await.unwrap();
        let mut vack = String::new();
        {
            let mut reader = BufReader::new(&mut victim);
            reader.read_line(&mut vack).await.unwrap();
        }
        assert_eq!(vack.trim(), "HELLO_OK", "la victime doit être acceptée");

        // L'attaquante tente de s'enregistrer sous le MÊME node_id —
        // le relay refuse et l'informe explicitement.
        let mut attacker = TcpStream::connect(addr).await.unwrap();
        attacker.write_all(b"HELLO bob\n").await.unwrap();
        let mut aack = String::new();
        {
            let mut reader = BufReader::new(&mut attacker);
            reader.read_line(&mut aack).await.unwrap();
        }
        assert_eq!(
            aack.trim(),
            "HELLO_DENIED",
            "l'attaquante doit être refusée"
        );

        let mut alice = TcpStream::connect(addr).await.unwrap();
        hello(&mut alice, "alice").await;

        let env = r#"{"kind":"fragment","from":"alice","to":"bob","session":"sH","seq":1,"type":"frag","idx":1,"threshold":4,"total":7,"payload":[4,2]}"#;
        alice
            .write_all(format!("{env}\n").as_bytes())
            .await
            .unwrap();

        // La VICTIME reçoit le fragment — son slot n'a pas été volé.
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(1), async {
            let mut reader = BufReader::new(&mut victim);
            reader.read_line(&mut line).await.unwrap();
        })
        .await
        .unwrap();
        assert!(
            line.contains("\"session\":\"sH\""),
            "la victime n'a rien reçu: {line}"
        );

        // L'attaquante, elle, ne reçoit RIEN (registration refusée, sa
        // connexion est fermée par le relay). EOF (Ok(0)) ou timeout =
        // « rien reçu » — seules des données réelles sont un échec.
        let mut aline = String::new();
        let got = tokio::time::timeout(Duration::from_millis(200), async {
            let mut reader = BufReader::new(&mut attacker);
            reader.read_line(&mut aline).await
        })
        .await;
        match got {
            Ok(Ok(0)) | Err(_) => {} // EOF ou timeout : l'attaquante n'a rien reçu ✓
            _ => panic!("l'attaquante a reçu le fragment: {aline}"),
        }
    }
}
