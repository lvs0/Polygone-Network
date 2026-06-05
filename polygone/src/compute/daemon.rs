//! Compute daemon: Polygone-Compute power lending
//!
//! A systemd-style daemon that:
//!   - Detects idle CPU/RAM resources
//!   - Automatically enables/disables power lending based on user activity
//!   - Integrates with Ollama for local inference sharing
//!   - Shows status in the TUI
//!
//! Smart detection: if user is active (keyboard/mouse input), pause lending.
//! When idle > 5 minutes, resume lending.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::idle::{IdleDetector, SystemMetrics};
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
            Self::Stopped           => "○ Stopped",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::LendingActive | Self::OllamaActive)
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
    /// Credits earned (rough estimate)
    pub credits_earned: f64,
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
            credits_earned: 0.0,
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
}

impl ComputeDaemon {
    pub fn new(config: ComputeConfig) -> Self {
        Self {
            idle_detector: IdleDetector::new(config.idle_threshold_sec),
            config,
            state: LendingState::Stopped,
            started_at: Instant::now(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            ollama_checked: false,
            ollama_models: Vec::new(),
            watts_used: 0.0,
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

        // Ollama active?
        if self.ollama_checked && !self.ollama_models.is_empty() {
            return LendingState::OllamaActive;
        }

        LendingState::LendingActive
    }

    /// Run the daemon until stopped.
    /// Returns when stop_flag is set or an error occurs.
    pub fn run(&mut self) -> Result<()> {
        use std::io::{self, Write};

        self.state = LendingState::LendingActive;

        println!("⬡ POLYGONE-COMPUTE — Power Lending Daemon");
        println!();
        println!("  Idle threshold : {:.0}s", self.config.idle_threshold_sec);
        println!("  Max RAM fraction: {:.0}%", self.config.max_ram_fraction * 100.0);
        println!("  Max CPU fraction: {:.0}%", self.config.max_cpu_fraction);
        println!("  Ollama enabled  : {}", self.config.ollama_enabled);
        println!("  Status listen   : {}", self.config.status_listen);
        println!();
        println!("  ✔ Lending daemon started — contributing resources to the network");
        println!("  Press Ctrl+C to stop gracefully.");
        println!();

        let poll = Duration::from_secs(self.config.poll_interval_sec);

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
            let new_state = self.compute_state(&metrics);
            let prev_state = self.state;
            self.state = new_state;

            // State transition logging
            if new_state != prev_state {
                match new_state {
                    LendingState::LendingActive => {
                        println!("  {} Lending resumed — system idle for {:.0}s",
                            chrono_display(), metrics.idle_seconds);
                    }
                    LendingState::PausedUserActive => {
                        println!("  {} Lending paused — user activity detected", chrono_display());
                    }
                    LendingState::PausedResourceLow => {
                        println!("  {} Lending paused — resources low ({:.0}% RAM, {:.0}% CPU)",
                            chrono_display(), metrics.ram_fraction()*100.0, metrics.cpu_usage);
                    }
                    LendingState::OllamaActive => {
                        println!("  {} Ollama inference sharing active", chrono_display());
                    }
                    LendingState::Stopped => {}
                }
                io::stdout().flush().ok();
            }

            // Update energy estimate
            if new_state.is_active() {
                self.watts_used += 15.0 * (poll.as_secs_f64() / 3600.0); // ~15W extra during lending
            }

            std::thread::sleep(poll);
        }

        self.state = LendingState::Stopped;
        println!();
        println!("  ✔ Lending daemon stopped cleanly.");
        Ok(())
    }

    /// Get current status snapshot.
    pub fn status(&self) -> ComputeStatus {
        let uptime = self.started_at.elapsed().as_secs();
        ComputeStatus {
            state: self.state,
            metrics: self.idle_detector.last_metrics(),
            uptime_secs: uptime,
            ollama_running: self.ollama_checked && !self.ollama_models.is_empty(),
            models_available: self.ollama_models.clone(),
            energy_contributed_j: (self.watts_used * 1000.0) as u64,
            credits_earned: self.watts_used * 0.001 * 10.0, // rough estimate: 10 credits per kJ
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
            // Check if process exists (Linux)
            return std::path::Path::new(&format!("/proc/{pid}")).exists();
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
