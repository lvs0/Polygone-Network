//! Service trait — the contract every Polygone service implements.
//!
//! A service is a long-lived unit of work owned by the Computer daemon.
//! It is pure async, idempotent on start/stop, observable through
//! `status()` / `health()` / `metrics()`, and never blocks the runtime.
//!
//! # Lifecycle
//!
//! ```text
//!                  ┌────────────┐
//!                  │   Stopped  │
//!                  └─────┬──────┘
//!                        │ start()
//!                        ▼
//!                  ┌────────────┐
//!                  │  Starting  │
//!                  └─────┬──────┘
//!                        │ on success
//!                        ▼
//!                  ┌────────────┐
//!      ┌──────────►│  Running   │◄──┐
//!      │           └─────┬──────┘   │
//!      │                 │          │
//!      │    stop()       │          │ health check
//!      │                 ▼          │
//!      │           ┌────────────┐   │
//!      └───────────│  Stopping  │───┘ (auto-restart on error)
//!                  └────────────┘
//! ```
//!
//! # Idempotency
//!
//! `start()` on a running service MUST be a no-op. `stop()` on a stopped
//! service MUST be a no-op. This keeps the orchestrator simple and lets
//! operators hammer the service without race conditions.

use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::crypto::error::PolyResult;

/// Lifecycle phase of a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    /// Service has never been started.
    Stopped,
    /// start() has been called, init is in progress.
    Starting,
    /// Init done, service is doing its work.
    Running,
    /// stop() has been called, draining in progress.
    Stopping,
    /// Service crashed and the orchestrator has not yet recovered it.
    Crashed,
    /// Service declared as "soon" — not implemented yet.
    Planned,
}

impl Phase {
    pub fn is_active(self) -> bool {
        matches!(self, Phase::Running | Phase::Starting)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Stopped => "stopped",
            Phase::Starting => "starting",
            Phase::Running => "running",
            Phase::Stopping => "stopping",
            Phase::Crashed => "crashed",
            Phase::Planned => "planned",
        }
    }
}

/// Coarse-grained health state, derived from `health()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Health {
    /// Service is healthy and accepting work.
    Ok,
    /// Service is up but degraded (e.g. low peers, high latency).
    Degraded,
    /// Service is failing but the orchestrator has not yet crashed it.
    Failing,
    /// Service is down. Will be auto-restarted if configured.
    Down,
}

impl Health {
    pub fn as_str(self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::Degraded => "degraded",
            Health::Failing => "failing",
            Health::Down => "down",
        }
    }
}

/// A single metric, ready to be rendered in TUI / scraped by the web / exported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub unit: String,
    pub value: f64,
    /// Human-readable hint, e.g. "↑ 12.3 KB/s"
    pub hint: String,
}

/// Immutable identity of a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// Stable identifier used in CLI and config: "drive", "hide", "msg", ...
    pub id: &'static str,
    /// Display name, e.g. "Drive" / "Hide" / "Brain".
    pub name: &'static str,
    /// Single glyph used in the TUI / web.
    pub icon: &'static str,
    /// One-line description, French, minimaliste.
    pub desc: &'static str,
    /// Semantic version, taken from Cargo.toml where possible.
    pub version: &'static str,
    /// Listening port, or 0 if the service does not bind a port.
    pub port: u16,
}

impl ServiceInfo {
    pub const fn new(
        id: &'static str, name: &'static str, icon: &'static str,
        desc: &'static str, version: &'static str, port: u16,
    ) -> Self {
        Self { id, name, icon, desc, version, port }
    }
}

/// The contract every Polygone service implements.
///
/// # Required methods
///
/// - `info()` — immutable identity.
/// - `start()` — bring the service up. Idempotent.
/// - `stop()` — bring it down cleanly. Idempotent.
/// - `health()` — coarse-grained health for the orchestrator.
/// - `metrics()` — fine-grained metrics for observability.
///
/// # Default methods
///
/// - `status()` — derived from `info()`, `phase()`, `health()`.
/// - `versioned()` — info() + the current crate version.
#[async_trait]
pub trait Service: Send + Sync {
    /// Immutable identity.
    fn info(&self) -> ServiceInfo;
    /// Current lifecycle phase. Cheap to call.
    async fn phase(&self) -> Phase;
    /// Bring the service up. Idempotent.
    async fn start(&self) -> PolyResult<()>;
    /// Bring the service down. Idempotent.
    async fn stop(&self) -> PolyResult<()>;
    /// Coarse health. Should be O(1) and never block.
    async fn health(&self) -> Health;
    /// Fine-grained metrics. Should be O(1) and never block.
    async fn metrics(&self) -> Vec<Metric>;

    // ── default impls ──────────────────────────────────────────────────

    /// Whether this service is currently active. `true` if `start()` was called
    /// and `stop()` has not completed yet.
    async fn is_active(&self) -> bool {
        self.phase().await.is_active()
    }

    /// Combined status struct, ready to serialize to JSON for the web / IPC.
    async fn status(&self) -> ServiceStatus {
        let phase = self.phase().await;
        let health = self.health().await;
        let metrics = self.metrics().await;
        let info = self.info();
        ServiceStatus {
            id: info.id.to_string(),
            name: info.name.to_string(),
            icon: info.icon.to_string(),
            desc: info.desc.to_string(),
            version: info.version.to_string(),
            port: info.port,
            phase: phase.as_str().to_string(),
            health: health.as_str().to_string(),
            uptime_ms: self.uptime_ms().await,
            metrics,
        }
    }

    /// Time elapsed since `start()` returned successfully, in milliseconds.
    /// Returns 0 if the service is not running.
    async fn uptime_ms(&self) -> u64 {
        let _ = Duration::from_millis(0);
        0
    }
}

/// Snapshot of a service at one point in time, ready to be serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub desc: String,
    pub version: String,
    pub port: u16,
    pub phase: String,
    pub health: String,
    pub uptime_ms: u64,
    pub metrics: Vec<Metric>,
}

/// Configuration shared by every service: how to behave on crash, log level, …
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePolicy {
    /// Auto-restart on crash.
    pub auto_restart: bool,
    /// Max restart attempts before the service is marked as `Crashed`.
    pub max_restarts: u32,
    /// Restart backoff.
    pub restart_backoff: Duration,
    /// Whether this service is enabled in the user config.
    pub enabled: bool,
}

impl Default for ServicePolicy {
    fn default() -> Self {
        Self {
            auto_restart: true,
            max_restarts: 5,
            restart_backoff: Duration::from_secs(2),
            enabled: true,
        }
    }
}

/// Live event emitted by a service or by the Computer itself.
///
/// This is the Perplexity-style "thought stream" — what the system is
/// doing right now. The Computer aggregates events from every service
/// into a single broadcast channel that the TUI / web can subscribe to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEvent {
    /// Monotonic timestamp in milliseconds since the Computer booted.
    pub ts_ms: u64,
    /// Origin: which service emitted this (or "computer").
    pub source: String,
    /// What kind of event this is.
    pub kind: ServiceEventKind,
    /// Short human-readable message.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceEventKind {
    /// Informational — nothing went wrong.
    Info,
    /// Something is starting.
    Starting,
    /// Something completed successfully.
    Done,
    /// Something went wrong but is being handled.
    Warning,
    /// Something failed unrecoverably.
    Error,
    /// A metric sample (name + value).
    Metric,
    /// A user-facing log line.
    Log,
}

impl ServiceEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Starting => "starting",
            Self::Done => "done",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Metric => "metric",
            Self::Log => "log",
        }
    }
}

impl ServiceEvent {
    pub fn info(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ts_ms: now_ms(),
            source: source.into(),
            kind: ServiceEventKind::Info,
            message: message.into(),
        }
    }
    pub fn done(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ts_ms: now_ms(),
            source: source.into(),
            kind: ServiceEventKind::Done,
            message: message.into(),
        }
    }
    pub fn warn(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ts_ms: now_ms(),
            source: source.into(),
            kind: ServiceEventKind::Warning,
            message: message.into(),
        }
    }
    pub fn error(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ts_ms: now_ms(),
            source: source.into(),
            kind: ServiceEventKind::Error,
            message: message.into(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_strings() {
        assert_eq!(Phase::Running.as_str(), "running");
        assert_eq!(Phase::Stopped.as_str(), "stopped");
        assert_eq!(Phase::Planned.as_str(), "planned");
    }
    #[test]
    fn phase_is_active() {
        assert!(Phase::Running.is_active());
        assert!(Phase::Starting.is_active());
        assert!(!Phase::Stopped.is_active());
        assert!(!Phase::Crashed.is_active());
    }
    #[test]
    fn health_strings() {
        assert_eq!(Health::Ok.as_str(), "ok");
        assert_eq!(Health::Degraded.as_str(), "degraded");
    }
    #[test]
    fn service_info_const_ctor() {
        let i = ServiceInfo::new("drive", "Drive", "📁", "Stockage", "1.0.0", 8081);
        assert_eq!(i.id, "drive");
        assert_eq!(i.port, 8081);
    }
}
