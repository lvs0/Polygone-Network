//! Polygone IPC — line-delimited JSON over a Unix domain socket.
//!
//! The Computer daemon listens on $POLYGONE_SOCKET. Clients (TUI, CLI)
//! connect, send a one-line JSON request, and read a one-line JSON
//! response. No persistent state, no streaming, no binary frames.
//!
//! # Request
//!
//! ```json
//! {"id": "abc", "op": "status"}
//! {"id": "abc", "op": "start", "service": "drive"}
//! {"id": "abc", "op": "stop",  "service": "drive"}
//! {"id": "abc", "op": "list"}
//! ```
//!
//! # Response
//!
//! ```json
//! {"id": "abc", "ok": true,  "data": {...}}
//! {"id": "abc", "ok": false, "error": "..."}
//! ```

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::computer::Computer;

/// A client → daemon request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Request {
    pub id: String,
    pub op: Op,
    #[serde(default)]
    pub service: Option<String>,
}

/// One of the four supported operations.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Status,
    List,
    Start,
    Stop,
}

/// A daemon → client response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(id: impl Into<String>, data: serde_json::Value) -> Self {
        Self { id: id.into(), ok: true, data: Some(data), error: None }
    }
    pub fn err(id: impl Into<String>, e: impl ToString) -> Self {
        Self { id: id.into(), ok: false, data: None, error: Some(e.to_string()) }
    }
}

/// Bind the Unix listener. Removes a stale socket file first.
pub async fn bind(path: impl AsRef<Path>) -> std::io::Result<UnixListener> {
    let path = path.as_ref();
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    UnixListener::bind(path)
}

/// Handle a single client connection. Line-delimited.
pub async fn handle(stream: UnixStream, computer: Arc<Computer>) -> std::io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() { continue; }
        let resp = dispatch(line, &computer).await;
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into());
        out.push('\n');
        write.write_all(out.as_bytes()).await?;
    }
    Ok(())
}

async fn dispatch(line: &str, computer: &Computer) -> Response {
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return Response::err("?", format!("parse error: {e}")),
    };
    let id = req.id.clone();
    match req.op {
        Op::Status => {
            let snap = computer.snapshot().await;
            let data = serde_json::to_value(snap).unwrap_or(serde_json::Value::Null);
            Response::ok(id, data)
        }
        Op::List => {
            let snap = computer.snapshot().await;
            let services: Vec<serde_json::Value> = snap.services.into_iter()
                .map(|s| serde_json::to_value(s).unwrap_or(serde_json::Value::Null))
                .collect();
            Response::ok(id, serde_json::json!({ "services": services }))
        }
        Op::Start => match req.service {
            Some(s) => match computer.start_one(&s).await {
                Ok(_)  => Response::ok(id, serde_json::json!({ "started": s })),
                Err(e) => Response::err(id, e),
            },
            None => Response::err(id, "missing 'service'"),
        },
        Op::Stop => match req.service {
            Some(s) => match computer.stop_one(&s).await {
                Ok(_)  => Response::ok(id, serde_json::json!({ "stopped": s })),
                Err(e) => Response::err(id, e),
            },
            None => Response::err(id, "missing 'service'"),
        },
    }
}

/// Convenience for clients: open a Unix socket, send one request, read one response.
pub async fn call(path: impl AsRef<Path>, req: &Request) -> std::io::Result<Response> {
    let mut s = UnixStream::connect(path).await?;
    let mut line = serde_json::to_string(req).unwrap_or_else(|_| "{}".into());
    line.push('\n');
    s.write_all(line.as_bytes()).await?;
    let mut buf = String::new();
    let (read, _write) = s.into_split();
    let mut r = BufReader::new(read);
    use tokio::io::AsyncBufReadExt;
    r.read_line(&mut buf).await?;
    let resp: Response = serde_json::from_str(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{Health, Metric, Phase, Service, ServiceInfo};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    struct Stub {
        id: &'static str,
        phase: RwLock<Phase>,
    }
    impl Stub {
        fn new(id: &'static str) -> Self { Self { id, phase: RwLock::new(Phase::Stopped) } }
    }
    #[async_trait]
    impl Service for Stub {
        fn info(&self) -> ServiceInfo {
            ServiceInfo::new(self.id, "Stub", "?", "t", "0", 0)
        }
        async fn phase(&self) -> Phase { *self.phase.read().await }
        async fn start(&self) -> crate::crypto::error::PolyResult<()> {
            *self.phase.write().await = Phase::Running; Ok(())
        }
        async fn stop(&self) -> crate::crypto::error::PolyResult<()> {
            *self.phase.write().await = Phase::Stopped; Ok(())
        }
        async fn health(&self) -> Health { Health::Ok }
        async fn metrics(&self) -> Vec<Metric> { vec![] }
    }

    async fn make_computer() -> Arc<Computer> {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Stub::new("stub"))).await.unwrap();
        c
    }

    #[tokio::test]
    async fn dispatch_status_ok() {
        let c = make_computer().await;
        let resp = dispatch(r#"{"id":"1","op":"status"}"#, &c).await;
        assert_eq!(resp.id, "1");
        assert!(resp.ok);
        assert!(resp.error.is_none());
    }

    #[tokio::test]
    async fn dispatch_list_returns_array() {
        let c = make_computer().await;
        let resp = dispatch(r#"{"id":"2","op":"list"}"#, &c).await;
        assert!(resp.ok);
        let v = resp.data.unwrap();
        assert!(v["services"].is_array());
        assert_eq!(v["services"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn dispatch_start_and_stop() {
        let c = make_computer().await;
        let r1 = dispatch(r#"{"id":"a","op":"start","service":"stub"}"#, &c).await;
        assert!(r1.ok);
        let r2 = dispatch(r#"{"id":"b","op":"stop","service":"stub"}"#, &c).await;
        assert!(r2.ok);
    }

    #[tokio::test]
    async fn dispatch_unknown_op_returns_error() {
        let c = make_computer().await;
        let r = dispatch(r#"{"id":"x","op":"restart","service":"stub"}"#, &c).await;
        assert!(!r.ok);
        assert!(r.error.is_some());
    }

    #[tokio::test]
    async fn dispatch_start_missing_service() {
        let c = make_computer().await;
        let r = dispatch(r#"{"id":"y","op":"start"}"#, &c).await;
        assert!(!r.ok);
    }

    #[tokio::test]
    async fn dispatch_parse_error() {
        let c = make_computer().await;
        let r = dispatch("not-json", &c).await;
        assert!(!r.ok);
    }
}
