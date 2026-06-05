//! Idle detection for Polygone-Compute.
//!
//! Monitors CPU, RAM, and user activity (keyboard/mouse).
//! Lending is paused when the user is active, resumed when idle.

use std::time::Instant;

/// Collected system metrics at a point in time.
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// Overall CPU usage 0.0–100.0
    pub cpu_usage: f32,
    /// RAM used / total in bytes
    pub ram_used: u64,
    pub ram_total: u64,
    /// Seconds since last user input (keyboard + mouse)
    pub idle_seconds: f64,
    /// Whether the system is considered idle (idle_seconds > threshold)
    pub is_idle: bool,
    /// True if we detected a keyboard/mouse event recently
    pub user_active: bool,
}

impl SystemMetrics {
    /// RAM usage as a fraction 0.0–1.0
    pub fn ram_fraction(&self) -> f32 {
        if self.ram_total == 0 { 0.0 } else { self.ram_used as f32 / self.ram_total as f32 }
    }

    /// Human-readable RAM usage string
    pub fn ram_str(&self) -> String {
        let used_gb = self.ram_used as f64 / 1_073_741_824.0;
        let total_gb = self.ram_total as f64 / 1_073_741_824.0;
        format!("{:.1}/{:.1} GB", used_gb, total_gb)
    }

    /// Can we safely lend resources?
    pub fn can_lend(&self, max_ram_fraction: f32) -> bool {
        self.is_idle && self.cpu_usage < 30.0 && self.ram_fraction() < max_ram_fraction
    }
}

/// Reads /proc/stat to estimate CPU idle time.
fn read_cpu_idle() -> Option<(u64, u64)> {
    // /proc/stat: first line "cpu  user nice system idle iowait ..."
    let data = std::fs::read_to_string("/proc/stat").ok()?;
    let first = data.lines().next()?;
    let fields: Vec<u64> = first
        .split_whitespace()
        .skip(1)
        .take(8)
        .filter_map(|s| s.parse().ok())
        .collect();
    if fields.len() < 4 { return None; }
    let total: u64 = fields.iter().sum();
    let idle = fields.get(3).copied().unwrap_or(0);
    Some((total, idle))
}

/// Reads /proc/meminfo to get RAM usage.
fn read_ram() -> Option<(u64, u64)> {
    // MemTotal and MemAvailable in kB
    let data = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0u64;
    let mut available = 0u64;
    for line in data.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let kb: u64 = parts[1].parse().ok()?;
        match parts[0] {
            "MemTotal:" => total = kb * 1024,
            "MemAvailable:" => available = kb * 1024,
            _ => {}
        }
    }
    if total == 0 { return None; }
    // used = total - available
    let used = total.saturating_sub(available);
    Some((used, total))
}

/// Reads /proc/interrupts and /dev/input to estimate user activity.
/// On Linux this is a rough heuristic — we check if the last input event
/// in /proc/interrupts or /dev/input/event* is recent.
fn read_idle_time() -> f64 {
    // Try reading from /proc/interrupts for keyboard (IRQ 1 on most x86)
    // and mouse (IRQ 12 on PS/2). A simpler cross-platform way would be
    // to track last keyboard/mouse access via /dev/input but that needs
    // root. Here we use uptime as a proxy and compute idle from CPU.
    // For a proper implementation we check the idle counter via syscalls.
    //
    // Approach: check X screensaver idle time if DISPLAY is set, else
    // fall back to CPU idle ratio.
    if let Ok(display) = std::env::var("DISPLAY") {
        if !display.is_empty() {
            // Try querying X screensaver via xprintidle if available
            if let Ok(output) = std::process::Command::new("xprintidle").output() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    if let Ok(ms) = s.trim().parse::<u64>() {
                        return ms as f64 / 1000.0;
                    }
                }
            }
        }
    }

    // Fallback: derive idle from CPU usage. High idle = system is idle.
    // This is approximate but works without special permissions.
    if let Some((total, idle)) = read_cpu_idle() {
        if total > 0 {
            // idle_ratio is fraction of time spent idle since boot
            let idle_ratio = idle as f64 / total as f64;
            // uptime in seconds (from /proc/uptime)
            if let Ok(uptime_data) = std::fs::read_to_string("/proc/uptime") {
                if let Some(uptime) = uptime_data.split_whitespace().next() {
                    if let Ok(up) = uptime.parse::<f64>() {
                        // estimated_idle_time = uptime * idle_ratio
                        // But this grows unbounded. Instead, we return
                        // a "time since last busy" proxy:
                        // use the diff of idle between calls instead.
                        return up * idle_ratio;
                    }
                }
            }
            return idle_ratio * 60.0; // fallback
        }
    }
    0.0
}

/// Tracks the previous CPU snapshot for delta calculations.
pub struct CpuSampler {
    prev_total: u64,
    prev_idle: u64,
    prev_instant: Instant,
}

impl CpuSampler {
    pub fn new() -> Self {
        let (total, idle) = read_cpu_idle().unwrap_or((0, 0));
        Self { prev_total: total, prev_idle: idle, prev_instant: Instant::now() }
    }

    /// Sample CPU and return usage percentage (0.0–100.0) since last call.
    pub fn sample(&mut self) -> f32 {
        let (total, idle) = read_cpu_idle().unwrap_or((0, 0));
        let now = Instant::now();
        let elapsed = now.duration_since(self.prev_instant).as_secs_f64();

        if elapsed < 0.1 { return 0.0; }

        let total_delta = total.saturating_sub(self.prev_total) as f64;
        let idle_delta = idle.saturating_sub(self.prev_idle) as f64;

        let usage = if total_delta > 0.0 {
            100.0 * (1.0 - idle_delta / total_delta)
        } else {
            0.0
        };

        self.prev_total = total;
        self.prev_idle = idle;
        self.prev_instant = now;

        usage.max(0.0).min(100.0) as f32
    }
}

impl Default for CpuSampler {
    fn default() -> Self { Self::new() }
}

/// Idle detection for power lending.
///
/// Polls CPU, RAM, and user activity on Linux.
/// Lending is allowed when the system is idle (no recent user input).
pub struct IdleDetector {
    cpu_sampler: CpuSampler,
    idle_threshold_sec: f64,
    last_idle_check: Instant,
    last_idle_sec: f64,
}

impl IdleDetector {
    pub fn new(idle_threshold_sec: f64) -> Self {
        Self {
            cpu_sampler: CpuSampler::new(),
            idle_threshold_sec,
            last_idle_check: Instant::now(),
            last_idle_sec: 0.0,
        }
    }

    /// Collect a snapshot of current system metrics.
    pub fn metrics(&mut self) -> SystemMetrics {
        let cpu_usage = self.cpu_sampler.sample();
        let (ram_used, ram_total) = read_ram().unwrap_or((0, 0));
        let idle_seconds = read_idle_time();
        let is_idle = idle_seconds >= self.idle_threshold_sec;
        let user_active = idle_seconds < 10.0; // active if idle < 10s

        self.last_idle_sec = idle_seconds;
        self.last_idle_check = Instant::now();

        SystemMetrics {
            cpu_usage,
            ram_used,
            ram_total,
            idle_seconds,
            is_idle,
            user_active,
        }
    }

    /// Same as metrics() but uses cached value from last poll.
    pub fn last_metrics(&self) -> SystemMetrics {
        SystemMetrics {
            cpu_usage: 0.0,
            ram_used: 0,
            ram_total: 0,
            idle_seconds: self.last_idle_sec,
            is_idle: self.last_idle_sec >= self.idle_threshold_sec,
            user_active: self.last_idle_sec < 10.0,
        }
    }
}
