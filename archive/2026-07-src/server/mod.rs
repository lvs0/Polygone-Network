//! Polygone Server — a stateless, zero-knowledge relay.
//!
//! The server NEVER sees plaintext. It only forwards opaque encrypted
//! fragments between nodes that can not reach each other directly
//! (NAT, firewall, mobile network).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_FRAGMENT_BYTES: usize = 32 * 1024;
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct RelayStore {
    inner: RwLock<HashMap<String, (Vec<u8>, Instant)>>,
}

impl RelayStore {
    pub fn new() -> Arc<Self> { Arc::new(Self::default()) }

    pub async fn put(&self, bytes: Vec<u8>) -> String {
        let token = new_token();
        let mut g = self.inner.write().await;
        g.insert(token.clone(), (bytes, Instant::now() + DEFAULT_TTL));
        token
    }

    pub async fn get(&self, token: &str) -> Option<Vec<u8>> {
        let mut g = self.inner.write().await;
        if let Some((bytes, deadline)) = g.get(token) {
            if Instant::now() < *deadline {
                return Some(bytes.clone());
            }
        }
        g.remove(token);
        None
    }

    pub async fn sweep(&self) -> usize {
        let now = Instant::now();
        let mut g = self.inner.write().await;
        let before = g.len();
        g.retain(|_, (_, d)| *d > now);
        before - g.len()
    }

    pub async fn len(&self) -> usize { self.inner.read().await.len() }
}

fn parse_request(buf: &[u8]) -> Option<(&str, &str, Vec<u8>)> {
    let s = std::str::from_utf8(buf).ok()?;
    let mut lines = s.lines();
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let path   = parts.next()?;
    let mut content_length = 0usize;
    let mut found_blank = false;
    for line in lines {
        if line.is_empty() { found_blank = true; continue; }
        if found_blank { break; }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().unwrap_or(0);
        }
    }
    let body = if let Some(idx) = s.find("\r\n\r\n") {
        let raw = &s.as_bytes()[idx + 4..];
        raw.iter().take(content_length).copied().collect()
    } else {
        Vec::new()
    };
    Some((method, path, body))
}

fn text(status: u16, status_text: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).into_bytes()
}

fn json(status: u16, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).into_bytes()
}

fn not_found() -> Vec<u8> { text(404, "Not Found", "") }
fn bad_request(msg: &str) -> Vec<u8> { text(400, "Bad Request", msg) }

async fn handle_conn(
    mut stream: TcpStream, store: Arc<RelayStore>, stats: Arc<RelayStats>,
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 65536];
    let n = match stream.read(&mut buf).await {
        Ok(0) => return Ok(()),
        Ok(n) => n,
        Err(_) => return Ok(()),
    };
    let (method, path, body) = match parse_request(&buf[..n]) {
        Some(t) => t,
        None => {
            stream.write_all(&bad_request("malformed request")).await?;
            return Ok(());
        }
    };
    let response = route(method, path, body, store, stats).await;
    stream.write_all(&response).await?;
    Ok(())
}

async fn route(
    method: &str, path: &str, body: Vec<u8>,
    store: Arc<RelayStore>, stats: Arc<RelayStats>,
) -> Vec<u8> {
    if method == "GET" && path == "/health" {
        return text(200, "OK", "polygone-server");
    }
    if method == "GET" && path == "/stats" {
        let s = stats.snapshot(store.len().await).await;
        let body = serde_json::to_string(&s).unwrap_or_else(|_| "{}".to_string());
        return json(200, &body);
    }
    if method == "POST" && path == "/relay" {
        if body.is_empty() || body.len() > MAX_FRAGMENT_BYTES {
            return bad_request("fragment must be 1..=32 KB");
        }
        let token = store.put(body).await;
        stats.put_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return json(201, &format!(r#"{{"id":"{token}"}}"#));
    }
    if method == "GET" && path.starts_with("/relay/") {
        let id = &path[7..];
        if id.is_empty() || id.contains('/') {
            return bad_request("invalid id");
        }
        return match store.get(id).await {
            Some(bytes) => {
                stats.get_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mut resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                ).into_bytes();
                resp.extend_from_slice(&bytes);
                resp
            }
            None => not_found(),
        };
    }
    not_found()
}

use std::sync::atomic::AtomicU64;

#[derive(Default)]
pub struct RelayStats {
    pub put_count: AtomicU64,
    pub get_count: AtomicU64,
    pub served_conns: AtomicU64,
}

impl RelayStats {
    pub fn new() -> Arc<Self> { Arc::new(Self::default()) }

    pub async fn snapshot(&self, in_memory: usize) -> RelayStatsSnapshot {
        RelayStatsSnapshot {
            version: VERSION.to_string(),
            put_count: self.put_count.load(std::sync::atomic::Ordering::Relaxed),
            get_count: self.get_count.load(std::sync::atomic::Ordering::Relaxed),
            served_conns: self.served_conns.load(std::sync::atomic::Ordering::Relaxed),
            in_memory,
            max_fragment_bytes: MAX_FRAGMENT_BYTES,
            ttl_secs: DEFAULT_TTL.as_secs(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct RelayStatsSnapshot {
    pub version: String,
    pub put_count: u64,
    pub get_count: u64,
    pub served_conns: u64,
    pub in_memory: usize,
    pub max_fragment_bytes: usize,
    pub ttl_secs: u64,
}

pub async fn serve(bind: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind).await?;
    eprintln!("[polygone-server] listening on http://{bind}");
    let store = RelayStore::new();
    let stats = RelayStats::new();

    {
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                let dropped = store.sweep().await;
                if dropped > 0 {
                    eprintln!("[polygone-server] gc dropped {dropped} fragment(s)");
                }
            }
        });
    }

    loop {
        let (stream, _peer) = listener.accept().await?;
        let store = Arc::clone(&store);
        let stats = Arc::clone(&stats);
        stats.served_conns.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, store, stats).await {
                eprintln!("[polygone-server] conn error: {e}");
            }
        });
    }
}

fn new_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos()).unwrap_or(0);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in nanos.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn put_and_get() {
        let s = RelayStore::new();
        let t = s.put(b"hello world".to_vec()).await;
        assert_eq!(s.get(&t).await, Some(b"hello world".to_vec()));
    }

    #[tokio::test]
    async fn missing_returns_none() {
        let s = RelayStore::new();
        assert!(s.get("zzz").await.is_none());
    }

    #[tokio::test]
    async fn parse_get_health() {
        let req = b"GET /health HTTP/1.1\r\nHost: x\r\n\r\n";
        let (m, p, b) = parse_request(req).unwrap();
        assert_eq!(m, "GET");
        assert_eq!(p, "/health");
        assert!(b.is_empty());
    }

    #[tokio::test]
    async fn parse_post_with_body() {
        let req = b"POST /relay HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
        let (m, p, b) = parse_request(req).unwrap();
        assert_eq!(m, "POST");
        assert_eq!(p, "/relay");
        assert_eq!(b, b"hello");
    }

    #[tokio::test]
    async fn token_is_hex_16() {
        let t = new_token();
        assert_eq!(t.len(), 16);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
