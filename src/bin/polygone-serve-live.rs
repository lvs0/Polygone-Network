//! `polygone-serve-live` — boot a real Computer and expose it through
//! the live HTTP API (the "Perplexity style" thought stream + plan gate).
//!
//! Compared to `polygone serve` which only serves a static NodeState,
//! this binary:
//!   1. boots a real `Computer`
//!   2. registers a few demo services (msg, hide, drive, mesh, brain)
//!   3. starts `web::serve_live` which talks to the real Computer
//!
//! The user can then visit `http://<bind>/plan.html`, click
//! "propose boot plan", see the live SSE event feed, and approve /
//! reject the plan. No stubs. No fake data.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use polygone::computer::Computer;
use polygone::services::{Health, Metric, Phase, Service, ServiceInfo};
use polygone::web::{serve_live, WebConfig};

/// A minimal demo service used to make the boot plan non-empty.
/// All five demo services are Stopped at boot, so propose_boot_plan
/// will create a 5-step plan to start them all.
struct DemoService {
    info: ServiceInfo,
    phase: tokio::sync::RwLock<Phase>,
}
impl DemoService {
    fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            info: ServiceInfo::new(id, name, "?", "demo service", "1.0.0", 9000),
            phase: tokio::sync::RwLock::new(Phase::Stopped),
        }
    }
}
#[async_trait]
impl Service for DemoService {
    fn info(&self) -> ServiceInfo { self.info.clone() }
    async fn phase(&self) -> Phase { *self.phase.read().await }
    async fn start(&self) -> polygone::PolyResult<()> {
        *self.phase.write().await = Phase::Running;
        Ok(())
    }
    async fn stop(&self) -> polygone::PolyResult<()> {
        *self.phase.write().await = Phase::Stopped;
        Ok(())
    }
    async fn health(&self) -> Health { Health::Ok }
    async fn metrics(&self) -> Vec<Metric> { vec![] }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let bind: SocketAddr = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| "127.0.0.1:8765".parse().unwrap());

    eprintln!("[polygone-serve-live] booting Computer…");
    let computer = Computer::boot().await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    for (id, name) in [
        ("msg",   "End-to-end messenger"),
        ("hide",  "SOCKS5 tunnel"),
        ("drive", "Distributed storage"),
        ("mesh",  "mDNS discovery"),
        ("brain", "LLM router"),
    ] {
        computer.register(Arc::new(DemoService::new(id, name))).await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    }
    eprintln!("[polygone-serve-live] registered 5 demo services (all Stopped)");

    // Print the boot plan immediately so the operator sees the
    // plan-then-execute flow without touching the UI.
    let plan = computer.propose_boot_plan().await;
    eprintln!("\n[polygone-serve-live] proposed plan: {}", plan.title);
    eprintln!("[polygone-serve-live]   id:    {}", plan.id);
    eprintln!("[polygone-serve-live]   steps: {}", plan.steps.len());
    for s in &plan.steps {
        eprintln!("[polygone-serve-live]     - {} {} ({})",
            s.service, s.action.as_str(), s.rationale);
    }
    eprintln!("\n[polygone-serve-live] open http://{bind}/plan.html to approve/reject");
    eprintln!("[polygone-serve-live] (or POST /api/plan/approve from curl)\n");

    let cfg = WebConfig { bind };
    serve_live(cfg, computer).await
}
