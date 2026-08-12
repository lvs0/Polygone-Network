//! hide — Polygone Hide (HIDE-SPEC.md Phase 1): SOCKS5 proxy through the
//! blind relay.
//!
//! Two roles:
//! - **Client** (`polygone hide --via <relay> --sortie <node> -d <pk>`):
//!   a local SOCKS5 listener (127.0.0.1:9050). Every CONNECT is tunnelled
//!   through the relay to the exit node, which opens the real connection.
//! - **Exit node** (`polygone ecouter --hide`): receives `req` envelopes
//!   with an encrypted destination, opens the real TCP connection, replies
//!   a grant, then relays `stream` envelopes both ways.
//!
//! Security model (honest, see HIDE-SPEC.md):
//! - The relay sees only `kind`/`to`/`session`/sizes — never the destination
//!   (encrypted with ML-KEM) nor the content (AES-256-GCM).
//! - The exit node sees the real destination (like a Tor exit) — documented.
//! - Single hop (MVP): relay + exit node together could correlate — said
//!   out loud, no Tor-level promise.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use polygone_core::crypto::{kem, symmetric};

use crate::identity::LocalIdentity;
use crate::net;

/// The default SOCKS5 listen address (projected port, docs/config.md).
pub const DEFAULT_LISTEN: &str = "127.0.0.1:9050";
/// Timeout for the exit node to answer a CONNECT (grant).
const GRANT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// Timeout for the exit node's real TCP connection to the destination.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

// ─────────────────────────────────────────────────────────────────────────────
// SOCKS5 (RFC 1928) — client side
// ─────────────────────────────────────────────────────────────────────────────

/// A parsed SOCKS5 CONNECT request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5Connect {
    pub host: String,
    pub port: u16,
}

/// Parse a SOCKS5 CONNECT request. Returns the destination host/port.
///
/// RFC 1928: `VER CMD RSV ATYP DST.ADDR DST.PORT`
/// - VER=0x05, CMD=0x01 (CONNECT), RSV=0x00
/// - ATYP: 0x01 IPv4, 0x03 domain, 0x04 IPv6
pub fn parse_socks5_connect(buf: &[u8]) -> Result<Socks5Connect> {
    if buf.len() < 4 || buf[0] != 0x05 {
        anyhow::bail!("pas un handshake SOCKS5 (VER != 5)");
    }
    if buf[1] != 0x01 {
        anyhow::bail!("seul CONNECT (0x01) est supporté");
    }
    if buf[2] != 0x00 {
        anyhow::bail!("RSV doit être 0x00");
    }
    let atyp = buf[3];
    match atyp {
        0x01 => {
            if buf.len() < 10 {
                anyhow::bail!("adresse IPv4 tronquée");
            }
            let host = format!("{}.{}.{}.{}", buf[4], buf[5], buf[6], buf[7]);
            let port = u16::from_be_bytes([buf[8], buf[9]]);
            Ok(Socks5Connect { host, port })
        }
        0x03 => {
            if buf.len() < 5 {
                anyhow::bail!("domaine tronqué");
            }
            let len = buf[4] as usize;
            if buf.len() < 5 + len + 2 {
                anyhow::bail!("domaine tronqué (len {len})");
            }
            let host = String::from_utf8_lossy(&buf[5..5 + len]).to_string();
            let port = u16::from_be_bytes([buf[5 + len], buf[6 + len]]);
            Ok(Socks5Connect { host, port })
        }
        0x04 => {
            if buf.len() < 22 {
                anyhow::bail!("adresse IPv6 tronquée");
            }
            let ip = std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&buf[4..20])?);
            let port = u16::from_be_bytes([buf[20], buf[21]]);
            Ok(Socks5Connect {
                host: ip.to_string(),
                port,
            })
        }
        other => anyhow::bail!("ATYP inconnu : 0x{other:02x}"),
    }
}

/// The SOCKS5 success reply (bound addr 0.0.0.0:0).
pub fn socks5_success() -> Vec<u8> {
    vec![0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]
}

/// The SOCKS5 failure reply (general failure = 0x01).
pub fn socks5_failure() -> Vec<u8> {
    vec![0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0]
}

/// Read and answer the SOCKS5 greeting (`05 <nmethods> <methods>`).
async fn socks5_greeting<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin>(
    reader: &mut R,
    writer: &mut W,
) -> Result<()> {
    let mut head = [0u8; 2];
    reader.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        anyhow::bail!("SOCKS5 : VER != 5");
    }
    let nmethods = head[1] as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;
    // Reply: no authentication required (0x00).
    writer.write_all(&[0x05, 0x00]).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a full SOCKS5 CONNECT request (bounded to 262 bytes: 4 + 255 + 2).
async fn read_connect_request<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut head = [0u8; 4];
    reader.read_exact(&mut head).await?;
    let atyp = head[3];
    let addr_len = match atyp {
        0x01 => 4,
        0x03 => {
            let mut len = [0u8; 1];
            reader.read_exact(&mut len).await?;
            len[0] as usize
        }
        0x04 => 16,
        _ => anyhow::bail!("SOCKS5 : ATYP inconnu 0x{atyp:02x}"),
    };
    let mut rest = vec![0u8; addr_len + 2];
    reader.read_exact(&mut rest).await?;
    let mut buf = Vec::with_capacity(4 + rest.len());
    buf.extend_from_slice(&head);
    if atyp == 0x03 {
        buf.push(addr_len as u8);
    }
    buf.extend_from_slice(&rest);
    Ok(buf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Client : SOCKS5 listener → tunnel
// ─────────────────────────────────────────────────────────────────────────────

/// Run the client: SOCKS5 listener on `listen`, each CONNECT tunnelled to
/// `exit_node` through `relay`, using `exit_pk_hex` (ML-KEM public key of
/// the exit node) for the encrypted destination.
pub async fn serve(
    relay: &str,
    exit_node: &str,
    exit_pk_hex: &str,
    listen: &str,
    identity: LocalIdentity,
) -> Result<()> {
    let exit_pk = kem::KemPublicKey::from_hex(exit_pk_hex)?;
    let listener = TcpListener::bind(listen).await?;
    println!("⬡ hide : SOCKS5 sur {listen} — tunnel → {exit_node} via relay {relay}");
    println!("  (Ctrl-C pour arrêter)");

    loop {
        let (sock, _peer) = listener.accept().await?;
        let relay = relay.to_string();
        let exit_node = exit_node.to_string();
        let exit_pk = exit_pk.clone();
        let identity = identity.clone();
        tokio::spawn(async move {
            if let Err(e) =
                handle_socks_connection(sock, &relay, &exit_node, &exit_pk, &identity).await
            {
                println!("  ✖ hide : {e}");
            }
        });
    }
}

/// Handle one SOCKS5 client connection end to end.
async fn handle_socks_connection(
    mut sock: TcpStream,
    relay: &str,
    exit_node: &str,
    exit_pk: &kem::KemPublicKey,
    identity: &LocalIdentity,
) -> Result<()> {
    let (mut r, mut w) = sock.split();
    socks5_greeting(&mut r, &mut w).await?;
    let req_buf = read_connect_request(&mut r).await?;
    let target = parse_socks5_connect(&req_buf)?;

    // Open the tunnel through the relay to the exit node.
    let channel = match net::hide_establish(
        relay,
        exit_node,
        exit_pk,
        identity,
        &target.host,
        target.port,
        GRANT_TIMEOUT,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            // The application is waiting for an answer — tell it the
            // connection failed (RFC 1928 REP 0x01, general failure).
            let _ = w.write_all(&socks5_failure()).await;
            anyhow::bail!("tunnel refusé : {e}");
        }
    };

    // Tunnel established — tell the application it can go.
    w.write_all(&socks5_success()).await?;
    w.flush().await?;
    println!(
        "  ⬡ tunnel ouvert : {}:{} → {exit_node} (session {})",
        target.host, target.port, channel.session
    );

    // Bidirectional relay:
    //   app → (encrypt) → relay → exit node
    //   app ← (decrypt) ← relay ← exit node
    let (mut app_r, mut app_w) = sock.into_split();
    let relay_writer = Arc::new(Mutex::new(channel.writer));
    let key = channel.key.clone();
    let session = channel.session.clone();
    let to = exit_node.to_string();
    let from = net::node_id(identity);

    // Task 1: app → exit node (read the SOCKS client, encrypt, forward).
    let up_writer = relay_writer.clone();
    let up_key = key.clone();
    let up_session = session.clone();
    let up_to = to.clone();
    let up_from = from.clone();
    let up = tokio::spawn(async move {
        let mut buf = vec![0u8; net::STREAM_CHUNK];
        loop {
            let n = match app_r.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let ct = match net::hide_encrypt_chunk(&buf[..n], &up_key) {
                Ok(c) => c,
                Err(_) => break,
            };
            let env = net::stream_envelope(&up_session, &up_from, &up_to, &ct);
            let line = match net::envelope_line(&env) {
                Ok(l) => l,
                Err(_) => break,
            };
            let mut w = up_writer.lock().await;
            if w.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
        // EOF marker: tell the exit node the client side is done.
        let env = net::stream_envelope(&up_session, &up_from, &up_to, &[]);
        if let Ok(line) = net::envelope_line(&env) {
            let mut w = up_writer.lock().await;
            let _ = w.write_all(line.as_bytes()).await;
        }
    });

    // Task 2: exit node → app (read stream envelopes, decrypt, write).
    let mut down_reader = channel.reader;
    let down_key = key.clone();
    let down_session = session.clone();
    let down = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            line.clear();
            let n = match down_reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if n == 0 {
                break;
            }
            let env: net::NetEnvelope = match serde_json::from_str(line.trim()) {
                Ok(e) => e,
                Err(_) => continue,
            };
            if env.typ != "stream" || env.session != down_session {
                continue;
            }
            match net::hide_decrypt_chunk(&env.payload, &down_key) {
                Ok(Some(data)) => {
                    if app_w.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break, // EOF from the exit side
                Err(_) => break,
            }
        }
    });

    let _ = tokio::join!(up, down);
    println!("  ⬡ tunnel fermé (session {})", channel.session);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Exit node : receive hide requests, open the real connection, relay streams
// ─────────────────────────────────────────────────────────────────────────────

/// The exit node loop: connect to the relay, answer `req` hide envelopes by
/// opening the real TCP connection, then relay `stream` envelopes both ways.
pub async fn exit_listen(relay: &str, identity: &LocalIdentity) -> Result<()> {
    let stream = net::connect_relay(relay, identity).await?;
    let (reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(writer));
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut known_peers = net::load_peers();
    // Active tunnels: session → (session key, real-connection write half).
    let mut tunnels: HashMap<String, (symmetric::SessionKey, OwnedWriteHalf)> = HashMap::new();

    println!(
        "⬡ hide : nœud de sortie actif via relay {relay} — node {}",
        net::node_id(identity)
    );
    println!("  (Ctrl-C pour arrêter)");

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            println!("relay déconnecté.");
            return Ok(());
        }
        if line.starts_with("HELLO_DENIED") {
            println!("✖ ce node_id est déjà connecté au relay (slot pris).");
            return Ok(());
        }
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let env: net::NetEnvelope = match serde_json::from_str(line.trim()) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Incoming data for an active tunnel → decrypt, write to the target.
        if env.typ == "stream" {
            if let Some((key, tcp_w)) = tunnels.get_mut(&env.session) {
                match net::hide_decrypt_chunk(&env.payload, key) {
                    Ok(Some(data)) => {
                        if tcp_w.write_all(&data).await.is_err() {
                            // The target closed — drop the tunnel.
                            tunnels.remove(&env.session);
                        }
                    }
                    Ok(None) => {
                        // EOF from the client → close the target side.
                        tunnels.remove(&env.session);
                    }
                    Err(_) => {
                        // Corrupted chunk → fail closed, drop the tunnel.
                        tunnels.remove(&env.session);
                    }
                }
            }
            continue;
        }

        // A hide request addressed to us.
        if env.typ == "req" && env.to == net::node_id(identity) {
            // Authenticate the sender (signed request, fresh timestamp,
            // trust anchor). Only provable senders get a tunnel.
            if !net::verify_req(&env, &mut known_peers, now_secs) {
                println!("  ✖ hide : requête non authentifiée de {}", env.from);
                continue;
            }
            net::save_peers(&known_peers);

            // Decapsulate: first CT_SIZE bytes are the ML-KEM ciphertext,
            // the rest is the AES-GCM-encrypted destination.
            let ct = match kem::KemCiphertext::from_bytes(&env.payload[..kem::CT_SIZE]) {
                Ok(c) => c,
                Err(_) => {
                    println!("  ✖ hide : KEM ciphertext invalide de {}", env.from);
                    continue;
                }
            };
            let shared = match kem::decapsulate(&identity.kem_secret_key()?, &ct) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let key = symmetric::SessionKey::derive_from_secret(&shared);
            let plain = match symmetric::decrypt(&env.payload[kem::CT_SIZE..], &key) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let body: serde_json::Value = match serde_json::from_slice(&plain) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let host = match body.get("host").and_then(|h| h.as_str()) {
                Some(h) => h.to_string(),
                None => continue,
            };
            let port = match body.get("port").and_then(|p| p.as_u64()) {
                Some(p) if p <= u16::MAX as u64 => p as u16,
                _ => continue,
            };

            // Open the real connection. Fail-closed: a refused connection
            // is answered with an encrypted `ok:false` grant.
            let target = match tokio::time::timeout(
                CONNECT_TIMEOUT,
                TcpStream::connect((host.as_str(), port)),
            )
            .await
            {
                Ok(Ok(t)) => t,
                _ => {
                    println!(
                        "  ✖ hide : connexion impossible vers {host}:{port} (depuis {})",
                        env.from
                    );
                    let grant_payload = match symmetric::encrypt(br#"{"ok":false}"#, &key) {
                        Ok(p) => p,
                        Err(_) => continue,
                    };
                    let grant = net::grant_envelope(
                        &env.session,
                        &net::node_id(identity),
                        &env.from,
                        &grant_payload,
                    );
                    if let Ok(line) = net::envelope_line(&grant) {
                        let mut w = writer.lock().await;
                        let _ = w.write_all(line.as_bytes()).await;
                    }
                    continue;
                }
            };

            // Grant: encrypted with the session key — only the requester can
            // read it (the relay cannot forge it without the key).
            let grant_payload = match symmetric::encrypt(br#"{"ok":true}"#, &key) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let grant = net::grant_envelope(
                &env.session,
                &net::node_id(identity),
                &env.from,
                &grant_payload,
            );
            let line = net::envelope_line(&grant)?;
            {
                let mut w = writer.lock().await;
                w.write_all(line.as_bytes()).await?;
            }

            println!(
                "  ⬡ hide : tunnel accordé à {} → {}:{} (session {})",
                env.from, host, port, env.session
            );

            // Split the real connection; keep the write half for incoming
            // stream data, spawn a task for the return direction.
            let (mut tcp_r, tcp_w) = target.into_split();
            tunnels.insert(env.session.clone(), (key.clone(), tcp_w));

            // Task: real target → encrypt → stream envelopes back to client.
            let up_writer = writer.clone();
            let up_key = key.clone();
            let up_session = env.session.clone();
            let up_from = net::node_id(identity);
            let up_to = env.from.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; net::STREAM_CHUNK];
                loop {
                    let n = match tcp_r.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    let ct = match net::hide_encrypt_chunk(&buf[..n], &up_key) {
                        Ok(c) => c,
                        Err(_) => break,
                    };
                    let env = net::stream_envelope(&up_session, &up_from, &up_to, &ct);
                    let line = match net::envelope_line(&env) {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    let mut w = up_writer.lock().await;
                    if w.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
                // EOF: tell the client the target side is done.
                let env = net::stream_envelope(&up_session, &up_from, &up_to, &[]);
                if let Ok(line) = net::envelope_line(&env) {
                    let mut w = up_writer.lock().await;
                    let _ = w.write_all(line.as_bytes()).await;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socks5_parse_ipv4() {
        // CONNECT 1.2.3.4:8080
        let req = [0x05, 0x01, 0x00, 0x01, 1, 2, 3, 4, 0x1f, 0x90];
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(c.host, "1.2.3.4");
        assert_eq!(c.port, 8080);
    }

    #[test]
    fn socks5_parse_domain() {
        // CONNECT example.com:443
        let mut req = vec![0x05, 0x01, 0x00, 0x03, 11];
        req.extend_from_slice(b"example.com");
        req.extend_from_slice(&[0x01, 0xbb]); // 443
        let c = parse_socks5_connect(&req).unwrap();
        assert_eq!(c.host, "example.com");
        assert_eq!(c.port, 443);
    }

    #[test]
    fn socks5_reject_bad_ver() {
        assert!(parse_socks5_connect(&[0x04, 0x01, 0x00, 0x01]).is_err());
    }

    #[test]
    fn socks5_reject_bind() {
        // CMD=0x02 (BIND) is not supported.
        assert!(parse_socks5_connect(&[0x05, 0x02, 0x00, 0x01]).is_err());
    }

    #[test]
    fn socks5_replies_have_correct_shape() {
        assert_eq!(socks5_success().len(), 10);
        assert_eq!(socks5_success()[1], 0x00);
        assert_eq!(socks5_failure()[1], 0x01);
    }
}
