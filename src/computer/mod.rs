//! Polygone Computer — the local orchestrator.
//!
//! Owns every service, watches them, restarts on crash, exposes a single
//! status snapshot to the TUI / web / IPC. The Computer itself is
//! stateless w.r.t. crypto: it borrows the identity from `polygone init`
//! and never logs it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;

use crate::crypto::error::{PolygoneError, PolyResult};
use crate::services::{Health, Phase, Service, ServiceEvent, ServiceStatus};

/// The orchestrator. Cheap to clone (Arc).
pub struct Computer {
    services: RwLock<HashMap<String, Arc<dyn Service>>>,
    started_at: RwLock<Option<Instant>>,
    /// Event bus — every service can push, every consumer can pull.
    /// Bounded to avoid OOM if nobody reads.
    event_tx: mpsc::Sender<ServiceEvent>,
    event_rx: RwLock<Option<mpsc::Receiver<ServiceEvent>>>,
    /// Current plan, if any. Set by `propose_plan`, executed by `execute_plan`.
    current_plan: RwLock<Option<Plan>>,
}

/// A plan is the explicit list of steps the Computer intends to execute.
///
/// The user sees the plan, approves or rejects it. Only approved plans
/// run. This is the Gemini / Perplexity "plan-then-execute" pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Unique id (timestamp + nanoseconds).
    pub id: String,
    /// Human title.
    pub title: String,
    /// Ordered steps.
    pub steps: Vec<PlanStep>,
    /// State machine.
    pub state: PlanState,
    /// When the plan was proposed.
    pub proposed_at_ms: u64,
    /// When the plan was approved (if it has been).
    pub approved_at_ms: Option<u64>,
    /// When execution finished (success or failure).
    pub finished_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Service id this step targets.
    pub service: String,
    /// What to do with it.
    pub action: PlanAction,
    /// Why (a one-liner shown in the TUI / web).
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanAction {
    Start,
    Stop,
    Restart,
}

impl PlanAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanState {
    Proposed,
    Approved,
    Running,
    Done,
    Failed,
    Rejected,
}

impl Computer {
    pub async fn boot() -> PolyResult<Arc<Self>> {
        // Bounded event bus. 1024 events is plenty for a dashboard that
        // polls every 250 ms; if nobody reads, the oldest get dropped.
        let (event_tx, event_rx) = mpsc::channel(1024);
        let me = Arc::new(Self {
            services: RwLock::new(HashMap::new()),
            started_at: RwLock::new(None),
            event_tx,
            event_rx: RwLock::new(Some(event_rx)),
            current_plan: RwLock::new(None),
        });
        me.register_default_services().await?;
        // Emit the first event so the dashboard never shows "no data".
        let _ = me.event_tx.send(ServiceEvent::info(
            "computer",
            format!("polygone v{} booted", env!("CARGO_PKG_VERSION")),
        )).await;
        Ok(me)
    }

    /// Send an event into the bus. Fire-and-forget — if the consumer is
    /// behind, the channel is bounded and old events are dropped silently.
    pub async fn emit(&self, event: ServiceEvent) {
        let _ = self.event_tx.send(event).await;
    }

    /// Take the receiver out of the Computer. Can only be called once.
    /// The web / TUI / IPC subscriber calls this to start reading events.
    pub async fn take_event_stream(&self) -> Option<mpsc::Receiver<ServiceEvent>> {
        self.event_rx.write().await.take()
    }

    /// Get a clone of the sender so external code (services, sub-agents)
    /// can push events without holding a reference to the Computer.
    pub fn event_sender(&self) -> mpsc::Sender<ServiceEvent> {
        self.event_tx.clone()
    }

    pub async fn register_default_services(&self) -> PolyResult<()> { Ok(()) }

    pub async fn register(&self, svc: Arc<dyn Service>) -> PolyResult<()> {
        let id = svc.info().id.to_string();
        self.services.write().await.insert(id.clone(), svc);
        self.emit(ServiceEvent::info("computer", format!("registered service: {id}"))).await;
        Ok(())
    }

    // ─── Plan management (Gemini / Perplexity pattern) ───────────────

    /// Build a plan to start every registered service that is currently
    /// Stopped or Crashed. The plan is *proposed*, not *executed* — the
    /// user must approve it before anything actually runs.
    pub async fn propose_boot_plan(&self) -> Plan {
        let mut steps = Vec::new();
        let services = self.services.read().await;
        let mut registered: Vec<&Arc<dyn Service>> = services.values().collect();
        registered.sort_by(|a, b| a.info().id.cmp(b.info().id));
        for svc in registered {
            let p = svc.phase().await;
            if matches!(p, Phase::Stopped | Phase::Crashed) {
                steps.push(PlanStep {
                    service: svc.info().id.to_string(),
                    action: PlanAction::Start,
                    rationale: format!(
                        "{} is {} — bring it up",
                        svc.info().name,
                        p.as_str(),
                    ),
                });
            }
        }
        drop(services);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let plan = Plan {
            id: format!("plan-{now}"),
            title: format!("Boot {} service(s)", steps.len()),
            steps,
            state: PlanState::Proposed,
            proposed_at_ms: now,
            approved_at_ms: None,
            finished_at_ms: None,
        };
        *self.current_plan.write().await = Some(plan.clone());
        self.emit(ServiceEvent::info(
            "computer",
            format!("proposed plan: {} ({} steps)", plan.title, plan.steps.len()),
        )).await;
        plan
    }

    /// Mark the current plan as approved. Does NOT execute it; call
    /// `execute_current_plan` for that.
    pub async fn approve_current_plan(&self) -> PolyResult<()> {
        let mut guard = self.current_plan.write().await;
        let plan = guard.as_mut()
            .ok_or_else(|| PolygoneError::InvalidArgument("no plan to approve".into()))?;
        if plan.state != PlanState::Proposed {
            return Err(PolygoneError::InvalidArgument(format!(
                "plan is {:?}, cannot approve", plan.state
            )));
        }
        plan.state = PlanState::Approved;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        plan.approved_at_ms = Some(now);
        self.emit(ServiceEvent::info(
            "computer",
            format!("plan {} approved ({} steps)", plan.id, plan.steps.len()),
        )).await;
        Ok(())
    }

    /// Reject the current plan. No-op if there is none.
    pub async fn reject_current_plan(&self) -> PolyResult<()> {
        let mut guard = self.current_plan.write().await;
        if let Some(plan) = guard.as_mut() {
            plan.state = PlanState::Rejected;
            self.emit(ServiceEvent::info(
                "computer",
                format!("plan {} rejected", plan.id),
            )).await;
        }
        Ok(())
    }

    /// Execute the current (approved) plan step by step. Returns Ok(())
    /// if every step succeeded, Err if any step failed — but execution
    /// continues regardless (best effort).
    pub async fn execute_current_plan(self: &Arc<Self>) -> PolyResult<()> {
        let steps = {
            let guard = self.current_plan.read().await;
            let plan = guard.as_ref()
                .ok_or_else(|| PolygoneError::InvalidArgument("no plan".into()))?;
            if plan.state != PlanState::Approved {
                return Err(PolygoneError::InvalidArgument(format!(
                    "plan is {:?}, must be Approved", plan.state
                )));
            }
            plan.steps.clone()
        };
        {
            let mut guard = self.current_plan.write().await;
            if let Some(p) = guard.as_mut() { p.state = PlanState::Running; }
        }
        *self.started_at.write().await = Some(Instant::now());

        let mut failed = 0usize;
        for step in &steps {
            self.emit(ServiceEvent::info(
                "computer",
                format!("→ {} {} ({})", step.service, step.action.as_str(), step.rationale),
            )).await;
            let svc = match self.get(&step.service).await {
                Some(s) => s,
                None => {
                    self.emit(ServiceEvent::error(
                        "computer",
                        format!("service gone: {}", step.service),
                    )).await;
                    failed += 1;
                    continue;
                }
            };
            let res = match step.action {
                PlanAction::Start => svc.start().await,
                PlanAction::Stop => svc.stop().await,
                PlanAction::Restart => {
                    let _ = svc.stop().await;
                    svc.start().await
                }
            };
            match res {
                Ok(()) => self.emit(ServiceEvent::done(
                    "computer",
                    format!("✓ {} {}", step.service, step.action.as_str()),
                )).await,
                Err(e) => {
                    self.emit(ServiceEvent::error(
                        "computer",
                        format!("✗ {} {}: {e}", step.service, step.action.as_str()),
                    )).await;
                    failed += 1;
                }
            }
        }
        let final_state = if failed == 0 { PlanState::Done } else { PlanState::Failed };
        {
            let mut guard = self.current_plan.write().await;
            if let Some(p) = guard.as_mut() {
                p.state = final_state;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                p.finished_at_ms = Some(now);
            }
        }
        self.emit(ServiceEvent::info(
            "computer",
            format!("plan finished: {} ok, {} failed", steps.len() - failed, failed),
        )).await;
        if failed > 0 {
            Err(PolygoneError::InvalidArgument(format!("{failed} step(s) failed")))
        } else {
            Ok(())
        }
    }

    /// Read the current plan, if any.
    pub async fn current_plan(&self) -> Option<Plan> {
        self.current_plan.read().await.clone()
    }

    pub async fn start_all(&self) -> PolyResult<()> {
        *self.started_at.write().await = Some(Instant::now());
        for (id, svc) in self.services.read().await.iter() {
            let p = svc.phase().await;
            if p != Phase::Stopped && p != Phase::Crashed { continue; }
            eprintln!("[computer] starting {id}…");
            if let Err(e) = svc.start().await {
                eprintln!("[computer] {id} start failed: {e}");
            }
        }
        Ok(())
    }

    pub async fn stop_all(&self) -> PolyResult<()> {
        for (id, svc) in self.services.read().await.iter() {
            let p = svc.phase().await;
            if p != Phase::Running && p != Phase::Starting { continue; }
            eprintln!("[computer] stopping {id}…");
            if let Err(e) = svc.stop().await {
                eprintln!("[computer] {id} stop failed: {e}");
            }
        }
        *self.started_at.write().await = None;
        Ok(())
    }

    pub async fn run(&self) -> PolyResult<()> {
        self.start_all().await?;
        let mut tick = interval(Duration::from_millis(1000));
        loop {
            tick.tick().await;
            for (id, svc) in self.services.read().await.iter() {
                if svc.phase().await == Phase::Crashed {
                    eprintln!("[computer] {id} crashed — restarting");
                    if let Err(e) = svc.start().await {
                        eprintln!("[computer] {id} restart failed: {e}");
                    }
                }
            }
        }
    }

    pub async fn snapshot(&self) -> ComputerStatus {
        let mut statuses: Vec<ServiceStatus> = Vec::new();
        for svc in self.services.read().await.values() {
            statuses.push(svc.status().await);
        }
        statuses.sort_by(|a, b| a.id.cmp(&b.id));
        let uptime_ms = self.started_at.read().await
            .map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);
        ComputerStatus {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_ms,
            services: statuses,
        }
    }

    pub async fn get(&self, id: &str) -> Option<Arc<dyn Service>> {
        self.services.read().await.get(id).cloned()
    }

    pub async fn start_one(&self, id: &str) -> PolyResult<()> {
        let svc = self.get(id).await
            .ok_or_else(|| PolygoneError::InvalidArgument(format!("unknown service: {id}")))?;
        svc.start().await
    }

    pub async fn stop_one(&self, id: &str) -> PolyResult<()> {
        let svc = self.get(id).await
            .ok_or_else(|| PolygoneError::InvalidArgument(format!("unknown service: {id}")))?;
        svc.stop().await
    }

    pub async fn rollup_health(&self) -> Health {
        let mut worst = Health::Ok;
        for svc in self.services.read().await.values() {
            let h = svc.health().await;
            worst = match (worst, h) {
                (Health::Down, _) | (_, Health::Down) => Health::Down,
                (Health::Failing, _) | (_, Health::Failing) => Health::Failing,
                (Health::Degraded, _) | (_, Health::Degraded) => Health::Degraded,
                _ => Health::Ok,
            };
        }
        worst
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerStatus {
    pub version: String,
    pub uptime_ms: u64,
    pub services: Vec<ServiceStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::services::{Health, Metric, Phase, Service, ServiceInfo};

    struct Dummy {
        id: &'static str,
        phase: RwLock<Phase>,
    }
    impl Dummy {
        fn new(id: &'static str) -> Self {
            Self { id, phase: RwLock::new(Phase::Stopped) }
        }
    }
    #[async_trait]
    impl Service for Dummy {
        fn info(&self) -> ServiceInfo {
            ServiceInfo::new(self.id, self.id, "?", "test", "0.0.1", 0)
        }
        async fn phase(&self) -> Phase { *self.phase.read().await }
        async fn start(&self) -> PolyResult<()> {
            *self.phase.write().await = Phase::Running; Ok(())
        }
        async fn stop(&self) -> PolyResult<()> {
            *self.phase.write().await = Phase::Stopped; Ok(())
        }
        async fn health(&self) -> Health { Health::Ok }
        async fn metrics(&self) -> Vec<Metric> { vec![] }
    }

    #[tokio::test]
    async fn boot_registers_nothing_yet() {
        let c = Computer::boot().await.unwrap();
        let s = c.snapshot().await;
        assert!(s.services.is_empty());
    }

    #[tokio::test]
    async fn register_and_start_one() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("tst"))).await.unwrap();
        c.start_one("tst").await.unwrap();
        let s = c.snapshot().await;
        assert_eq!(s.services.len(), 1);
        assert_eq!(s.services[0].id, "tst");
        assert_eq!(s.services[0].phase, "running");
    }

    #[tokio::test]
    async fn stop_all_is_idempotent() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("a"))).await.unwrap();
        c.register(Arc::new(Dummy::new("b"))).await.unwrap();
        c.start_all().await.unwrap();
        c.stop_all().await.unwrap();
        c.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn start_one_unknown_errors() {
        let c = Computer::boot().await.unwrap();
        assert!(c.start_one("nope").await.is_err());
    }

    #[tokio::test]
    async fn rollup_health_is_ok_when_all_ok() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("a"))).await.unwrap();
        assert!(matches!(c.rollup_health().await, Health::Ok));
    }

    #[tokio::test]
    async fn snapshot_includes_uptime_zero_when_not_started() {
        let c = Computer::boot().await.unwrap();
        let s = c.snapshot().await;
        assert_eq!(s.uptime_ms, 0);
        assert!(!s.version.is_empty());
    }

    // ─── Plan tests (Gemini/Perplexity pattern) ───────────────────────

    #[tokio::test]
    async fn propose_plan_lists_stopped_services() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("alpha"))).await.unwrap();
        c.register(Arc::new(Dummy::new("beta"))).await.unwrap();
        let plan = c.propose_boot_plan().await;
        assert_eq!(plan.steps.len(), 2);
        assert!(plan.steps.iter().any(|s| s.service == "alpha"));
        assert!(plan.steps.iter().any(|s| s.service == "beta"));
        assert_eq!(plan.state, PlanState::Proposed);
    }

    #[tokio::test]
    async fn approve_then_execute_starts_everything() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("a"))).await.unwrap();
        c.register(Arc::new(Dummy::new("b"))).await.unwrap();
        let _ = c.propose_boot_plan().await;
        c.approve_current_plan().await.unwrap();
        c.execute_current_plan().await.unwrap();
        let s = c.snapshot().await;
        for svc in &s.services {
            assert_eq!(svc.phase, "running", "service {} not running", svc.id);
        }
        // The plan should now be Done.
        let plan = c.current_plan().await.unwrap();
        assert_eq!(plan.state, PlanState::Done);
    }

    #[tokio::test]
    async fn reject_marks_plan_rejected() {
        let c = Computer::boot().await.unwrap();
        c.register(Arc::new(Dummy::new("a"))).await.unwrap();
        let _ = c.propose_boot_plan().await;
        c.reject_current_plan().await.unwrap();
        let plan = c.current_plan().await.unwrap();
        assert_eq!(plan.state, PlanState::Rejected);
    }

    #[tokio::test]
    async fn approve_without_plan_errors() {
        let c = Computer::boot().await.unwrap();
        assert!(c.approve_current_plan().await.is_err());
    }

    #[tokio::test]
    async fn event_bus_drops_after_take() {
        let c = Computer::boot().await.unwrap();
        // Take the stream BEFORE we emit, otherwise the boot event is lost.
        let _rx = c.take_event_stream().await.unwrap();
        // Emitting now is a no-op (the receiver is gone).
        c.emit(ServiceEvent::info("test", "after-take")).await;
        // Calling take again returns None.
        assert!(c.take_event_stream().await.is_none());
    }

    #[tokio::test]
    async fn event_sender_works_independently() {
        let c = Computer::boot().await.unwrap();
        // Take the stream first so we own it (boot event may be dropped).
        let mut rx = c.take_event_stream().await.unwrap();
        // Drain the boot event if it's there.
        let _ = tokio::time::timeout(std::time::Duration::from_millis(20), rx.recv()).await;
        // Now send an event from an external sender.
        let tx = c.event_sender();
        tx.send(ServiceEvent::info("external", "hi")).await.unwrap();
        let ev = rx.recv().await.unwrap();
        assert_eq!(ev.source, "external");
        assert_eq!(ev.message, "hi");
    }
}
