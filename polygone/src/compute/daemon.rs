//! Compute daemon: Polygone-Compute power lending
//!
//! A systemd-style daemon that:
//!   - Detects idle CPU/RAM resources
//!   - Automatically enables/disables power lending based on user activity
//!   - Integrates with Ollama for local inference sharing
//!   - Tracks POLY token income from lending
//!   - Supports stealth mode for invisible operation
//!   - Negotiates resource sharing with peers via protocol
//!   - Shows status in the TUI
//!
//! Smart detection: if user is active (keyboard/mouse input), pause lending.
//! When idle > 5 minutes, resume lending.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::idle::{IdleDetector, SystemMetrics};
use super::lending::{ResourceScheduler, ResourceLimits, LendingStats};
use super::stealth::{StealthMode, StealthConfig, stealth_log};
use super::protocol::{ComputeMessage, CapabilityAnnounce, now_ms};
use crate::economy;
use crate::Result;

/// Configuration for the compute daemon.
#[derive(Debug, Clone)]
pub struct ComputeConfig {
    /// Seconds of idle before lending starts (default: 300 = 5 minutes)
    pub idle_threshold_sec: f64,
    /// Maximum RAM fraction to use for lending (0.0–1.0, default: 0.5)
    pub max_ram_fraction: f32,
    /// Maximum CPU fraction to use for lending (0.0–100.0, default: 80.0)
    pub max_cpu_fraction: f32,
    /// Path to Ollama binary (default: "ollama")
    pub ollama_path: String,
    /// Whether Ollama integration is enabled
    pub ollama_enabled: bool,
    /// Polling interval in seconds
    pub poll_interval_sec: u64,
    /// Listen address for the compute status server
    pub status_listen: String,
    /// Stealth mode configuration
    pub stealth: StealthConfig,
    /// Resource lending limits
    pub resource_limits: ResourceLimits,
    /// Whether resource lending is enabled
    pub lending_enabled: bool,
    /// POLY earned per CPU core-hour (default: 10.0)
    pub poly_per_core_hour: f64,
    /// POLY earned per GB RAM-hour (default: 5.0)
    pub poly_per_gb_ram_hour: f64,
}

impl Default for ComputeConfig {
    fn default() -> Self {
        Self {
            idle_threshold_sec: 300.0, // 5 minutes
            max_ram_fraction: 0.5,
            max_cpu_fraction: 80.0,
            ollama_path: "ollama".to_string(),
            ollama_enabled: true,
            poll_interval_sec: 10,
            status_listen: "127.0.0.1:4002".to_string(),
            stealth: StealthConfig::default(),
            resource_limits: ResourceLimits::default(),
            lending_enabled: true,
            poly_per_core_hour: 10.0,
            poly_per_gb_ram_hour: 5.0,
        }
    }
}

/// The running state of the compute daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LendingState {
    /// User is active — lending paused
    PausedUserActive,
    /// System resources too low — lending paused
    PausedResourceLow,
    /// Lending active — contributing resources to network
    LendingActive,
    /// Ollama integration active
    OllamaActive,
    /// Stealth mode active — minimal footprint
    StealthActive,
    /// Daemon stopped
    Stopped,
}

impl LendingState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PausedUserActive  => "⏸ Paused (user active)",
            Self::PausedResourceLow => "⏸ Paused (resources low)",
            Self::LendingActive     => "● Lending",
            Self::OllamaActive      => "◆ Ollama Active",
            Self::StealthActive     => "👁 Stealth",
            Self::Stopped           => "○ Stopped",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::LendingActive | Self::OllamaActive | Self::StealthActive)
    }
}

/// The current status of the compute daemon.
#[derive(Debug, Clone)]
pub struct ComputeStatus {
    pub state: LendingState,
    pub metrics: SystemMetrics,
    pub uptime_secs: u64,
    pub ollama_running: bool,
    pub models_available: Vec<String>,
    /// Energy contributed (rough estimate in joules)
    pub energy_contributed_j: u64,
    /// POLY earned from lending
    pub poly_earned: f64,
    /// POLY spent on renting
    pub poly_spent: f64,
    /// Current POLY balance (from economy ledger)
    pub poly_balance: f64,
    /// Lending stats
    pub lending_stats: LendingStats,
    /// Whether stealth mode is active
    pub stealth_active: bool,
    /// Active resource allocations count
    pub active_allocations: u32,
}

impl Default for ComputeStatus {
    fn default() -> Self {
        Self {
            state: LendingState::Stopped,
            metrics: SystemMetrics {
                cpu_usage: 0.0,
                ram_used: 0,
                ram_total: 0,
                idle_seconds: 0.0,
                is_idle: false,
                user_active: true,
            },
            uptime_secs: 0,
            ollama_running: false,
            models_available: Vec::new(),
            energy_contributed_j: 0,
            poly_earned: 0.0,
            poly_spent: 0.0,
            poly_balance: 0.0,
            lending_stats: LendingStats::default(),
            stealth_active: false,
            active_allocations: 0,
        }
    }
}

/// The compute daemon itself.
pub struct ComputeDaemon {
    config: ComputeConfig,
    idle_detector: IdleDetector,
    state: LendingState,
    started_at: Instant,
    stop_flag: Arc<AtomicBool>,
    ollama_checked: bool,
    ollama_models: Vec<String>,
    /// Rough energy estimate (watts_used * uptime_seconds / 1000 = kJ)
    watts_used: f64,
    /// Resource scheduler for lending
    scheduler: ResourceScheduler,
    /// Stealth mode controller
    stealth: StealthMode,
    /// POLY ticker for economy tracking
    poly_ticker: economy::Ticker,
}

impl ComputeDaemon {
    pub fn new(config: ComputeConfig) -> Self {
        let scheduler = ResourceScheduler::new(config.resource_limits.clone());
        let stealth = StealthMode::new(config.stealth.clone());
        let poly_ticker = economy::Ticker::load();

        Self {
            idle_detector: IdleDetector::new(config.idle_threshold_sec),
            config,
            state: LendingState::Stopped,
            started_at: Instant::now(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            ollama_checked: false,
            ollama_models: Vec::new(),
            watts_used: 0.0,
            scheduler,
            stealth,
            poly_ticker,
        }
    }

    /// Returns true if Ollama is running and accessible.
    pub fn check_ollama(&mut self) -> bool {
        if !self.config.ollama_enabled {
            return false;
        }
        let output = std::process::Command::new(&self.config.ollama_path)
            .args(["list"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let models: Vec<String> = stdout
                    .lines()
                    .skip(1) // header line
                    .filter_map(|l| {
                        let name = l.split_whitespace().next()?;
                        if name.is_empty() || name == "NAME" { return None; }
                        Some(name.to_string())
                    })
                    .collect();
                self.ollama_models = models;
                true
            }
            _ => false,
        }
    }

    /// Determine the next lending state based on current metrics.
    fn compute_state(&mut self, metrics: &SystemMetrics) -> LendingState {
        if !metrics.is_idle {
            return LendingState::PausedUserActive;
        }

        // Check RAM threshold
        if metrics.ram_fraction() > self.config.max_ram_fraction {
            return LendingState::PausedResourceLow;
        }

        // Check CPU threshold
        if metrics.cpu_usage > self.config.max_cpu_fraction {
            return LendingState::PausedResourceLow;
        }

        // Stealth mode takes priority when enabled
        if self.config.stealth.enabled {
            return LendingState::StealthActive;
        }

        // Ollama active?
        if self.ollama_checked && !self.ollama_models.is_empty() {
            return LendingState::OllamaActive;
        }

        LendingState::LendingActive
    }

    /// Build a capability announcement for the network.
    fn build_announcement(&self, metrics: &SystemMetrics) -> CapabilityAnnounce {
        let total_ram_mb = metrics.ram_total / (1024 * 1024);
        let free_ram_mb = (metrics.ram_total - metrics.ram_used) / (1024 * 1024);
        let lendable_ram = (total_ram_mb as f32 * self.config.max_ram_fraction) as u64;

        CapabilityAnnounce {
            node_id: crate::identity::load_or_create().node_id_short,
            node_name: crate::identity::load_or_create().pseudo,
            available_ram_mb: free_ram_mb.min(lendable_ram),
            available_cpu_cores: ((100.0 - metrics.cpu_usage) / 100.0 * 8.0) as u32,
            available_storage_gb: 0, // not tracked yet
            available_gpu_units: 0,
            price_ram_per_mb_hour: self.config.poly_per_gb_ram_hour / 1024.0,
            price_cpu_per_core_hour: self.config.poly_per_core_hour,
            price_storage_per_gb_hour: 0.001,
            uptime_secs: self.started_at.elapsed().as_secs(),
            reputation: 85,
            timestamp_ms: now_ms(),
            ttl_secs: 300,
        }
    }

    /// Run the daemon until stopped.
    /// Returns when stop_flag is set or an error occurs.
    pub fn run(&mut self) -> Result<()> {
        use std::io::{self, Write};

        self.state = LendingState::LendingActive;

        // Initialize stealth mode if enabled
        self.stealth.initialize();

        let banner = if self.config.stealth.enabled && self.config.stealth.silent_mode {
            // In silent stealth mode, only log to file
            stealth_log(&self.config.stealth, "⬡ POLYGONE-COMPUTE — Power Lending Daemon (stealth mode)");
            stealth_log(&self.config.stealth, &format!("  Idle threshold: {:.0}s", self.config.idle_threshold_sec));
            stealth_log(&self.config.stealth, &format!("  Lending enabled: {}", self.config.lending_enabled));
            stealth_log(&self.config.stealth, &format!("  Status listen: {}", self.config.status_listen));
            stealth_log(&self.config.stealth, "  ✔ Stealth lending daemon started");
            false
        } else {
            println!("⬡ POLYGONE-COMPUTE — Power Lending Daemon");
            println!();
            println!("  Idle threshold : {:.0}s", self.config.idle_threshold_sec);
            println!("  Max RAM fraction: {:.0}%", self.config.max_ram_fraction * 100.0);
            println!("  Max CPU fraction: {:.0}%", self.config.max_cpu_fraction);
            println!("  Ollama enabled  : {}", self.config.ollama_enabled);
            println!("  Lending enabled : {}", self.config.lending_enabled);
            println!("  Stealth mode    : {}", self.config.stealth.enabled);
            println!("  Status listen   : {}", self.config.status_listen);
            println!();
            println!("  ✔ Lending daemon started — contributing resources to the network");
            println!("  Press Ctrl+C to stop gracefully.");
            println!();
            true
        };

        let poll = Duration::from_secs(self.config.poll_interval_sec);

        // Set low priority for lending processes
        super::lending::set_lending_nice();

        loop {
            // Check stop flag
            if self.stop_flag.load(Ordering::Relaxed) {
                break;
            }

            // Poll Ollama once at startup
            if !self.ollama_checked {
                self.ollama_checked = true;
                let _ = self.check_ollama();
            }

            // Get system metrics
            let metrics = self.idle_detector.metrics();

            // Stealth mode: throttle if CPU too high
            if self.config.stealth.enabled && self.stealth.should_throttle(metrics.cpu_usage) {
                // Back off — double the poll interval temporarily
                std::thread::sleep(Duration::from_secs(30));
                continue;
            }

            let new_state = self.compute_state(&metrics);
            let prev_state = self.state;
            self.state = new_state;

            // State transition logging
            if new_state != prev_state {
                let msg = match new_state {
                    LendingState::LendingActive => {
                        format!("  {} Lending resumed — system idle for {:.0}s",
                            chrono_display(), metrics.idle_seconds)
                    }
                    LendingState::PausedUserActive => {
                        format!("  {} Lending paused — user activity detected", chrono_display())
                    }
                    LendingState::PausedResourceLow => {
                        format!("  {} Lending paused — resources low ({:.0}% RAM, {:.0}% CPU)",
                            chrono_display(), metrics.ram_fraction()*100.0, metrics.cpu_usage)
                    }
                    LendingState::OllamaActive => {
                        format!("  {} Ollama inference sharing active", chrono_display())
                    }
                    LendingState::StealthActive => {
                        format!("  {} Stealth lending active — minimal footprint", chrono_display())
                    }
                    LendingState::Stopped => String::new(),
                };

                if self.config.stealth.silent_mode && self.config.stealth.enabled {
                    stealth_log(&self.config.stealth, &msg);
                } else if banner {
                    println!("{}", msg);
                    io::stdout().flush().ok();
                }
            }

            // Process resource scheduler
            if self.config.lending_enabled && new_state.is_active() {
                let new_allocs = self.scheduler.schedule(&metrics);
                for alloc in &new_allocs {
                    let msg = format!("  {} New allocation: {} ({} {})",
                        chrono_display(),
                        alloc.allocation_id,
                        alloc.request.amount,
                        alloc.request.resource_type.label()
                    );
                    if self.config.stealth.silent_mode && self.config.stealth.enabled {
                        stealth_log(&self.config.stealth, &msg);
                    } else if banner {
                        println!("{}", msg);
                    }
                }

                // Update POLY ticker with active allocation count
                let active_count = self.scheduler.active_allocations().len() as u32;
                self.poly_ticker.set_active(active_count);
            } else {
                // Cancel all allocations when not lending
                self.scheduler.cancel_all();
                self.poly_ticker.set_active(0);
            }

            // Tick the POLY economy (drain tokens for active services)
            let _balance = self.poly_ticker.tick();

            // Update energy estimate
            if new_state.is_active() {
                self.watts_used += 15.0 * (poll.as_secs_f64() / 3600.0); // ~15W extra during lending
            }

            // Stealth mode uses longer polling
            let actual_poll = if self.config.stealth.enabled {
                self.stealth.poll_interval()
            } else {
                poll
            };

            std::thread::sleep(actual_poll);
        }

        // Cleanup: cancel all allocations, stop poly ticker
        self.scheduler.cancel_all();
        self.poly_ticker.set_active(0);
        self.stealth.deactivate();
        self.state = LendingState::Stopped;

        let msg = "  ✔ Lending daemon stopped cleanly.";
        if self.config.stealth.silent_mode && self.config.stealth.enabled {
            stealth_log(&self.config.stealth, msg);
        } else {
            println!();
            println!("{}", msg);
        }
        Ok(())
    }

    /// Get current status snapshot.
    pub fn status(&self) -> ComputeStatus {
        let uptime = self.started_at.elapsed().as_secs();
        let poly_snapshot = self.poly_ticker.snapshot();
        let lending_stats = self.scheduler.stats().clone();

        ComputeStatus {
            state: self.state,
            metrics: self.idle_detector.last_metrics(),
            uptime_secs: uptime,
            ollama_running: self.ollama_checked && !self.ollama_models.is_empty(),
            models_available: self.ollama_models.clone(),
            energy_contributed_j: (self.watts_used * 1000.0) as u64,
            poly_earned: lending_stats.total_poly_earned,
            poly_spent: lending_stats.total_poly_spent,
            poly_balance: poly_snapshot.balance,
            lending_stats,
            stealth_active: self.stealth.is_active(),
            active_allocations: self.scheduler.active_allocations().len() as u32,
        }
    }

    /// Signal the daemon to stop.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    /// Get a stop flag clone for IPC.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    /// Get the resource scheduler (for external queries).
    pub fn scheduler(&self) -> &ResourceScheduler {
        &self.scheduler
    }

    /// Get the stealth mode state.
    pub fn is_stealth(&self) -> bool {
        self.stealth.is_active()
    }

    /// Get the lending stats.
    pub fn lending_stats(&self) -> &LendingStats {
        self.scheduler.stats()
    }
}

/// Lightweight timestamp for log output.
fn chrono_display() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

// ── CLI helpers ───────────────────────────────────────────────────────────────

/// Check if the compute daemon is running (by checking for a PID file).
pub fn daemon_is_running() -> bool {
    let pid_path = daemon_pid_path();
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            return super::idle::platform::pid_exists(pid);
        }
    }
    false
}

/// Write PID to the daemon PID file.
pub fn write_pid() -> std::io::Result<()> {
    let pid_path = daemon_pid_path();
    if let Some(parent) = std::path::Path::new(&pid_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&pid_path, std::process::id().to_string())
}

/// Remove the PID file.
pub fn remove_pid() -> std::io::Result<()> {
    std::fs::remove_file(daemon_pid_path())
}

pub fn daemon_pid_path() -> String {
    dirs::data_local_dir()
        .map(|d| d.join("polygone").join("compute.pid"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/tmp/polygone-compute.pid".to_string())
}
