//! Minimalist HTTP server for the Polygone node dashboard.
//! Pure tokio + std. No framework. ~300 lines. See lib.rs for the public API.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

use crate::VERSION;

pub mod assets;

use crate::computer::Computer;

#[derive(Debug, Default, Clone)]
pub struct NodeState {
    pub uptime_secs: u64,
    pub peer_count: u32,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub frag_ready: u32,
    pub frag_needed: u32,
    pub balance: f64,
    pub consumption: f64,
    pub modules: HashMap<String, String>,
    pub log: Vec<(String, String)>,
}

impl NodeState {
    pub fn fresh() -> Self {
        let mut modules = HashMap::new();
        modules.insert("msg".into(),   "running".into());
        modules.insert("hide".into(),  "running".into());
        modules.insert("drive".into(), "off".into());
        modules.insert("mesh".into(),  "off".into());
        modules.insert("brain".into(), "soon".into());
        Self {
            uptime_secs: 0, peer_count: 0,
            bytes_in: 0, bytes_out: 0,
            frag_ready: 3, frag_needed: 4,
            balance: 10.0, consumption: 0.1,
            modules,
            log: vec![
                ("success".into(), "Polygone v1.0.0 demarre".into()),
                ("info".into(),    "Cle ML-KEM-1024 generee".into()),
                ("info".into(),    "Tunnel Hide SOCKS5 ready :9050".into()),
            ],
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let log_json: Vec<serde_json::Value> = self.log.iter()
            .map(|(k, m)| serde_json::json!({"kind": k, "msg": m}))
            .collect();
        serde_json::json!({
            "version":      VERSION,
            "uptime_secs":  self.uptime_secs,
            "peer_count":   self.peer_count,
            "bytes_in":     self.bytes_in,
            "bytes_out":    self.bytes_out,
            "frag_ready":   self.frag_ready,
            "frag_needed":  self.frag_needed,
            "balance":      self.balance,
            "consumption":  self.consumption,
            "modules":      self.modules,
            "log":          log_json,
        })
    }
}

#[derive(Debug, Clone)]
pub struct WebConfig { pub bind: SocketAddr }

impl Default for WebConfig {
    fn default() -> Self {
        Self { bind: "127.0.0.1:8080".parse().expect("valid default bind") }
    }
}

pub async fn serve(cfg: WebConfig, state: Arc<RwLock<NodeState>>) -> std::io::Result<()> {
    let listener = TcpListener::bind(cfg.bind).await?;
    eprintln!("[web] listening on http://{}", cfg.bind);
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, state).await {
                eprintln!("[web] {peer}: {e}");
            }
        });
    }
}

/// Live HTTP server bound to a real Computer daemon.
///
/// Unlike `serve()` which uses a static `NodeState`, this reads the
/// current plan from the Computer, calls the Computer to propose /
/// approve / reject plans, and streams real ServiceEvents through
/// Server-Sent Events.
///
/// Endpoints:
/// - `GET  /api/plan`        — current plan (or idle placeholder)
/// - `POST /api/plan/propose`— build a new plan
/// - `POST /api/plan/approve`— approve the current plan
/// - `POST /api/plan/reject` — reject the current plan
/// - `GET  /api/events`      — SSE stream of ServiceEvent
/// - `GET  /api/status`      — Computer snapshot
/// - all static assets from `assets.rs`
pub async fn serve_live(cfg: WebConfig, computer: Arc<Computer>) -> std::io::Result<()> {
    let listener = TcpListener::bind(cfg.bind).await?;
    eprintln!("[web-live] listening on http://{}", cfg.bind);
    loop {
        let (stream, peer) = listener.accept().await?;
        let computer = Arc::clone(&computer);
        tokio::spawn(async move {
            if let Err(e) = handle_conn_live(stream, computer).await {
                eprintln!("[web-live] {peer}: {e}");
            }
        });
    }
}

async fn handle_conn_live(mut stream: TcpStream, computer: Arc<Computer>) -> std::io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 { return Ok(()); }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path   = parts.next().unwrap_or("/").to_string();
    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    let response = route_live(&method, &path, &body, &computer).await;
    stream.write_all(&response).await?;
    stream.flush().await?;
    Ok(())
}

async fn route_live(method: &str, path: &str, _body: &str, computer: &Arc<Computer>) -> Vec<u8> {
    if method == "GET" && (path == "/" || path == "/index.html") {
        return serve_static("index.html");
    }
    if method == "GET" && path == "/health" {
        return text(200, "OK", "polygone-live");
    }
    if method == "GET" && path == "/api/status" {
        let s = computer.snapshot().await;
        let body = serde_json::to_string(&s).unwrap_or_else(|_| "{}".into());
        return json(200, &body);
    }
    if method == "GET" && path == "/api/plan" {
        match computer.current_plan().await {
            Some(plan) => {
                let body = serde_json::to_string(&plan).unwrap_or_else(|_| "{}".into());
                return json(200, &body);
            }
            None => return json(200, r#"{"id":null,"state":"idle","title":null,"steps":[]}"#),
        }
    } else if method == "POST" && path == "/api/plan/propose" {
        let plan = computer.propose_boot_plan().await;
        let body = serde_json::to_string(&plan).unwrap_or_else(|_| "{}".into());
        return json(202, &body);
    } else if method == "POST" && path == "/api/plan/approve" {
        return match computer.approve_current_plan().await {
            Ok(()) => json(202, r#"{"ok":true}"#),
            Err(e) => json(409, &format!(r#"{{"ok":false,"error":"{e}"}}"#)),
        };
    } else if method == "POST" && path == "/api/plan/reject" {
        return match computer.reject_current_plan().await {
            Ok(()) => json(202, r#"{"ok":true}"#),
            Err(e) => json(409, &format!(r#"{{"ok":false,"error":"{e}"}}"#)),
        };
    } else if method == "GET" && path == "/api/events" {
        return sse_live(computer).await;
    } else if method == "GET" {
        return serve_static(path.trim_start_matches('/'));
    }
    text(405, "Method Not Allowed", "use GET or POST")
}

/// Real SSE: drains the Computer's event bus until 64 events or 30s.
async fn sse_live(computer: &Arc<Computer>) -> Vec<u8> {
    let mut rx = match computer.take_event_stream().await {
        Some(rx) => rx,
        None => return sse_heartbeat(), // fallback if stream was already taken
    };
    let header = "HTTP/1.1 200 OK\r\n\
                  Content-Type: text/event-stream\r\n\
                  Cache-Control: no-cache\r\n\
                  Connection: close\r\n\
                  Access-Control-Allow-Origin: *\r\n\
                  \r\n";
    let mut out = Vec::new();
    out.extend_from_slice(header.as_bytes());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let ready = format!(
        "event: ready\ndata: {{\"now_ms\":{now},\"source\":\"computer\",\"message\":\"polygone live event stream ready\"}}\n\n"
    );
    out.extend_from_slice(ready.as_bytes());
    // Drain up to 64 events with a 200ms timeout each, then close.
    for _ in 0..64 {
        match tokio::time::timeout(
            std::time::Duration::from_millis(200),
            rx.recv(),
        ).await {
            Ok(Some(ev)) => {
                let body = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".into());
                let line = format!("event: {}\ndata: {body}\n\n", ev.kind.as_str());
                out.extend_from_slice(line.as_bytes());
            }
            _ => break,
        }
    }
    out
}

async fn handle_conn(mut stream: TcpStream, state: Arc<RwLock<NodeState>>) -> std::io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    if n == 0 { return Ok(()); }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path   = parts.next().unwrap_or("/").to_string();
    for line in lines { if line.is_empty() { break; } }
    let response = route(&method, &path, state).await;
    stream.write_all(&response).await?;
    stream.flush().await?;
    Ok(())
}

async fn route(method: &str, path: &str, state: Arc<RwLock<NodeState>>) -> Vec<u8> {
    if method == "GET" && (path == "/" || path == "/index.html") {
        return serve_static("index.html");
    }
    if method == "GET" && path == "/health" {
        return text(200, "OK", "polygone");
    }
    if method == "GET" && path == "/api/status" {
        let s = state.read().await.to_json();
        return json(200, &s.to_string());
    }
    if method == "GET" && path == "/api/peers" {
        return json(200, r#"[{"id":"p1","name":"self","addr":"127.0.0.1:4001"}]"#);
    }
    // ── Plan / Events (Perplexity-style thought stream) ────────────
    if method == "GET" && path == "/api/plan" {
        let body = r#"{
            "id": null,
            "state": "idle",
            "title": null,
            "steps": [],
            "proposed_at_ms": null,
            "approved_at_ms": null,
            "finished_at_ms": null,
            "_note": "Computer daemon not running. Start `polygone-computer` to get a live plan."
        }"#;
        return json(200, body);
    }
    if method == "POST" && path == "/api/plan/propose" {
        let body = r#"{"id":"plan-pending","state":"proposed","title":"Would propose a boot plan","steps":[],"_note":"POST handler stub — wire to Computer::propose_boot_plan"}"#;
        return json(202, body);
    }
    if method == "POST" && path == "/api/plan/approve" {
        let body = r#"{"ok":true,"_note":"POST handler stub — wire to Computer::approve_current_plan"}"#;
        return json(202, body);
    }
    if method == "POST" && path == "/api/plan/reject" {
        let body = r#"{"ok":true,"_note":"POST handler stub — wire to Computer::reject_current_plan"}"#;
        return json(202, body);
    }
    if method == "GET" && path == "/api/events" {
        // Server-Sent Events. We send a heartbeat then close.
        // The real implementation will drain the Computer event bus.
        return sse_heartbeat();
    }
    if method == "POST" && path == "/api/share" {
        let token = new_token();
        let link = format!("/s/{token}");
        let body = serde_json::json!({ "url": link, "expires_in": 86400 });
        return json(201, &body.to_string());
    }
    if method == "GET" && path.starts_with("/s/") {
        return text(404, "Not Found", "share link expired or not found");
    }
    if method == "GET" {
        return serve_static(path.trim_start_matches('/'));
    }
    text(405, "Method Not Allowed", "use GET or POST")
}

fn serve_static(name: &str) -> Vec<u8> {
    if name.is_empty() || name.contains("..") || name.contains('/') {
        return text(400, "Bad Request", "invalid path");
    }
    match assets::get(name) {
        Some(bytes) => {
            let mime = mime_for(name);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            let mut out = Vec::with_capacity(header.len() + bytes.len());
            out.extend_from_slice(header.as_bytes());
            out.extend_from_slice(bytes);
            out
        }
        None => text(404, "Not Found", "asset not found"),
    }
}

fn mime_for(name: &str) -> &'static str {
    if name.ends_with(".html") { "text/html; charset=utf-8" }
    else if name.ends_with(".css") { "text/css; charset=utf-8" }
    else if name.ends_with(".js")  { "application/javascript; charset=utf-8" }
    else if name.ends_with(".svg") { "image/svg+xml" }
    else if name.ends_with(".json"){ "application/json; charset=utf-8" }
    else if name.ends_with(".png") { "image/png" }
    else if name.ends_with(".jpg") || name.ends_with(".jpeg") { "image/jpeg" }
    else { "application/octet-stream" }
}

fn text(status: u16, status_text: &str, body: &str) -> Vec<u8> {
    let s = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.into_bytes()
}

fn json(status: u16, body: &str) -> Vec<u8> {
    let s = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.into_bytes()
}

fn new_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in nanos.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Build a SSE heartbeat stream. The browser will reconnect
/// automatically; this is a poor-man's push channel that proves the
/// protocol works. The real implementation will subscribe to the
/// Computer's event bus.
fn sse_heartbeat() -> Vec<u8> {
    let header = "HTTP/1.1 200 OK\r\n\
                  Content-Type: text/event-stream\r\n\
                  Cache-Control: no-cache\r\n\
                  Connection: close\r\n\
                  Access-Control-Allow-Origin: *\r\n\
                  \r\n";
    let mut out = Vec::new();
    out.extend_from_slice(header.as_bytes());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // Send one ready event, one heartbeat, then close.
    let ready = format!(
        "event: ready\ndata: {{\"now_ms\":{now},\"source\":\"computer\",\"message\":\"polygone event stream ready (stub)\"}}\n\n"
    );
    out.extend_from_slice(ready.as_bytes());
    let beat = format!(
        ":heartbeat {now}\n\n"
    );
    out.extend_from_slice(beat.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_json_has_expected_keys() {
        let s = NodeState::fresh();
        let j = s.to_json();
        assert_eq!(j["frag_needed"], 4);
        assert_eq!(j["frag_ready"], 3);
        assert_eq!(j["modules"]["msg"], "running");
    }
    #[test]
    fn mime_guess_html() {
        assert_eq!(mime_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(mime_for("style.css"), "text/css; charset=utf-8");
        assert_eq!(mime_for("app.js"), "application/javascript; charset=utf-8");
    }
    #[test]
    fn path_traversal_rejected() {
        assert!(serve_static("../etc/passwd").starts_with(b"HTTP/1.1 400"));
        assert!(serve_static("").starts_with(b"HTTP/1.1 400"));
        assert!(serve_static("a/b").starts_with(b"HTTP/1.1 400"));
    }
    #[test]
    fn token_is_hex_16() {
        let t = new_token();
        assert_eq!(t.len(), 16);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn sse_heartbeat_starts_with_event_stream() {
        let out = sse_heartbeat();
        let head = std::str::from_utf8(&out[..60]).unwrap();
        assert!(head.contains("text/event-stream"));
        assert!(std::str::from_utf8(&out).unwrap().contains("event: ready"));
    }
    #[tokio::test]
    async fn route_plan_returns_idle_when_no_computer() {
        let s = Arc::new(RwLock::new(NodeState::fresh()));
        let r = route("GET", "/api/plan", s).await;
        let head = std::str::from_utf8(&r[..30]).unwrap();
        assert!(head.starts_with("HTTP/1.1 200"));
        let body = std::str::from_utf8(&r[r.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4..]).unwrap();
        // Pretty-printed JSON has spaces; just look for the key.
        assert!(body.contains("\"state\""));
        assert!(body.contains("\"idle\""));
        assert!(body.contains("Computer daemon not running"));
    }
    #[tokio::test]
    async fn route_events_is_sse() {
        let s = Arc::new(RwLock::new(NodeState::fresh()));
        let r = route("GET", "/api/events", s).await;
        let head = std::str::from_utf8(&r[..80]).unwrap();
        assert!(head.contains("text/event-stream"));
    }
}
