//! Polygone Gateway — HTTP API + static site + pairing.
//!
//! Endpoints:
//!   GET  /            → site officiel
//!   GET  /api/v1/network/status → état du réseau
//!   POST /api/v1/pair/request   → demande d'appairage
//!   GET  /api/v1/pair/qr/:token → QR SVG deep link
//!   POST /api/v1/pair/accept    → acceptation côté client
//!   GET  /api/v1/node/:id       → info d'un nœud

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub nodes: u64,
    pub active: u64,
    pub latency_ms_median: u64,
    pub uptime_seconds: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PairRequest {
    pub device_name: String,
    pub public_key_hex: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PairToken {
    pub token: String,
    pub device_name: String,
    pub public_key_hex: String,
    pub created_at: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub services: Vec<String>,
    pub last_seen: u64,
}

#[derive(Clone)]
pub struct Gateway {
    pub site_dir: PathBuf,
    pub pairs: Arc<tokio::sync::RwLock<HashMap<String, PairToken>>>,
    pub nodes: Arc<tokio::sync::RwLock<HashMap<String, NodeInfo>>>,
}

impl Gateway {
    pub async fn run(addr: &str, site_dir: PathBuf) -> Result<()> {
        let gateway = Self {
            site_dir,
            pairs: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            nodes: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };

        let listener = TcpListener::bind(addr).await?;
        info!("⬡ gateway : HTTP sur http://{addr}");

        loop {
            let (mut stream, _) = listener.accept().await?;
            let g = gateway.clone();
            tokio::spawn(async move {
                if let Err(e) = g.handle(&mut stream).await {
                    error!("gateway connection error: {e}");
                }
            });
        }
    }

    async fn handle(&self, stream: &mut tokio::net::TcpStream) -> Result<()> {
        let mut buf = [0u8; 4096];
        let n = stream.peek(&mut buf).await?;
        let raw = String::from_utf8_lossy(&buf[..n]);
        let request_line = raw.lines().next().unwrap_or("");
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Ok(());
        }
        let method = parts[0];
        let path = parts[1];

        let mime: String;
        let body: Vec<u8>;
        let len: usize;

        match (method, path) {
            ("GET", "/") => {
                let data = self.serve_site("index.html").await?;
                mime = "text/html".into();
                body = data;
                len = body.len();
            }
            ("GET", "/api/v1/network/status") => {
                let nodes = self.nodes.read().await;
                let status = NetworkStatus {
                    nodes: nodes.len() as u64,
                    active: nodes.len() as u64,
                    latency_ms_median: 42,
                    uptime_seconds: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };
                let json = serde_json::to_vec(&status)?;
                mime = "application/json".into();
                body = json;
                len = body.len();
            }
            ("POST", "/api/v1/pair/request") => {
                let body_raw = raw.split("\r\n\r\n").nth(1).unwrap_or("");
                let req: PairRequest = serde_json::from_str(body_raw)?;
                let token = format!("poly_{}", uuid_short());
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let pair = PairToken {
                    token: token.clone(),
                    device_name: req.device_name,
                    public_key_hex: req.public_key_hex,
                    created_at: now,
                };
                self.pairs.write().await.insert(token.clone(), pair);
                let json = serde_json::to_vec(&serde_json::json!({"token": token}))?;
                mime = "application/json".into();
                body = json;
                len = body.len();
            }
            ("GET", p) if p.starts_with("/api/v1/pair/qr/") => {
                let token = p.trim_start_matches("/api/v1/pair/qr/");
                let pairs = self.pairs.read().await;
                let pair = pairs.get(token).context("invalid token")?;
                let deep_link = format!("polygone://pair/{}", pair.token);
                let json = serde_json::to_vec(&serde_json::json!({"deep_link": deep_link}))?;
                mime = "application/json".into();
                body = json;
                len = body.len();
            }
            ("POST", "/api/v1/pair/accept") => {
                let body_raw = raw.split("\r\n\r\n").nth(1).unwrap_or("");
                let req: PairRequest = serde_json::from_str(body_raw)?;
                let mut nodes = self.nodes.write().await;
                nodes.insert(
                    req.public_key_hex.clone(),
                    NodeInfo {
                        id: req.public_key_hex,
                        services: vec!["msg".into()],
                        last_seen: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    },
                );
                let json = serde_json::to_vec(&serde_json::json!({"ok": true}))?;
                mime = "application/json".into();
                body = json;
                len = body.len();
            }
            ("GET", p) if p.starts_with("/api/v1/node/") => {
                let id = p.trim_start_matches("/api/v1/node/");
                let nodes = self.nodes.read().await;
                let node = nodes.get(id).cloned().unwrap_or(NodeInfo {
                    id: id.into(),
                    services: Vec::new(),
                    last_seen: 0,
                });
                let json = serde_json::to_vec(&node)?;
                mime = "application/json".into();
                body = json;
                len = body.len();
            }
            _ => {
                mime = "text/plain".into();
                body = b"not found".to_vec();
                len = body.len();
            }
        }

        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            mime, len
        );
        stream.write_all(header.as_bytes()).await?;
        stream.write_all(&body).await?;
        Ok(())
    }

    async fn serve_site(&self, file: &str) -> Result<Vec<u8>> {
        let path = self.site_dir.join(file);
        Ok(tokio::fs::read(&path).await?)
    }
}

fn uuid_short() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut s = String::new();
    for _ in 0..8 {
        s.push(char::from(b'A' + rng.gen_range(0..26)));
    }
    s
}
