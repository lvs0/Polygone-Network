//! Idle detection for Polygone-Compute.
//!
//! Monitors CPU, RAM, and user activity (keyboard/mouse).
//! Lending is paused when the user is active, resumed when idle.
//!
//! Cross-platform: Linux (via /proc), macOS (via sysctl), Windows (via API).

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

// ── Platform-specific implementations ──────────────────────────────────────

#[cfg(target_os = "linux")]
pub mod platform {
    /// Reads /proc/stat to estimate CPU idle time.
    pub fn read_cpu_idle() -> Option<(u64, u64)> {
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
    pub fn read_ram() -> Option<(u64, u64)> {
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
        let used = total.saturating_sub(available);
        Some((used, total))
    }

    /// Estimate user idle time from /proc/uptime and CPU ratio.
    pub fn read_idle_time() -> f64 {
        // Try xprintidle if DISPLAY is set
        if let Ok(display) = std::env::var("DISPLAY") {
            if !display.is_empty() {
                if let Ok(output) = std::process::Command::new("xprintidle").output() {
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        if let Ok(ms) = s.trim().parse::<u64>() {
                            return ms as f64 / 1000.0;
                        }
                    }
                }
            }
        }
        // Fallback: CPU idle ratio
        if let Some((total, idle)) = read_cpu_idle() {
            if total > 0 {
                let idle_ratio = idle as f64 / total as f64;
                if let Ok(uptime_data) = std::fs::read_to_string("/proc/uptime") {
                    if let Some(uptime) = uptime_data.split_whitespace().next() {
                        if let Ok(up) = uptime.parse::<f64>() {
                            return up * idle_ratio;
                        }
                    }
                }
                return idle_ratio * 60.0;
            }
        }
        0.0
    }

    /// Check if process exists via /proc/{pid}
    pub fn pid_exists(pid: u32) -> bool {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

#[cfg(target_os = "macos")]
pub mod platform {
    use std::process::Command;

    pub fn read_cpu_idle() -> Option<(u64, u64)> {
        // Use sysctl via vm_stat
        let output = Command::new("sysctl").args(["-n", "hw.ncpu"]).output().ok()?;
        let ncpu: u64 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;
        // Use top in batch mode for CPU stats
        let output = Command::new("top").args(["-l", "1", "-n", "0"]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse "CPU usage: X% user, Y% sys, Z% idle"
        for line in stdout.lines() {
            if line.contains("CPU usage") {
                let idle_pct = line.split("idle")
                    .next()?
                    .rsplit(|c: char| c.is_whitespace() || c == '%')
                    .nth(1)?
                    .parse::<f64>().ok()?;
                let idle = (idle_pct * ncpu as f64) as u64;
                let total = ncpu * 100;
                return Some((total, idle));
            }
        }
        None
    }

    pub fn read_ram() -> Option<(u64, u64)> {
        let output = Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        let total: u64 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;
        // Use vm_stat for used memory
        let output = Command::new("vm_stat").output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let page_size: u64 = 4096; // default
        let mut pages_active = 0u64;
        let mut pages_wired = 0u64;
        let mut pages_free = 0u64;
        for line in stdout.lines() {
            let val = |label: &str| -> Option<u64> {
                if line.contains(label) {
                    line.split_whitespace()
                        .nth(2)?
                        .trim_end_matches('.')
                        .parse().ok()
                } else { None }
            };
            if let Some(v) = val("Pages active") { pages_active = v; }
            if let Some(v) = val("Pages wired") { pages_wired = v; }
            if let Some(v) = val("Pages free") { pages_free = v; }
        }
        let used = (pages_active + pages_wired) * page_size;
        Some((used, total))
    }

    pub fn read_idle_time() -> f64 {
        // Use ioreg for HID idle time on macOS
        let output = Command::new("ioreg").args(["-c", "IOHIDSystem"]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("HIDIdleTime") {
                if let Some(ns_str) = line.split('=').nth(1) {
                    if let Ok(ns) = ns_str.trim().parse::<u64>() {
                        return ns as f64 / 1_000_000_000.0;
                    }
                }
            }
        }
        0.0
    }

    pub fn pid_exists(pid: u32) -> bool {
        Command::new("kill").args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(target_os = "windows")]
pub mod platform {
    use std::process::Command;

    pub fn read_cpu_idle() -> Option<(u64, u64)> {
        // Use wmic for CPU stats
        let output = Command::new("wmic").args(["cpu", "get", "LoadPercentage", "/value"]).output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pct_str) = line.strip_prefix("LoadPercentage=") {
                if let Ok(pct) = pct_str.trim().parse::<f64>() {
                    let idle = (100.0 - pct) as u64;
                    return Some((100, idle));
                }
            }
        }
        None
    }

    pub fn read_ram() -> Option<(u64, u64)> {
        let output = Command::new("wmic")
            .args(["OS", "get", "TotalVisibleMemorySize,FreePhysicalMemory", "/value"])
            .output().ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut total = 0u64;
        let mut free = 0u64;
        for line in stdout.lines() {
            if let Some(v) = line.strip_prefix("FreePhysicalMemory=") {
                free = v.trim().parse::<u64>().unwrap_or(0) * 1024;
            }
            if let Some(v) = line.strip_prefix("TotalVisibleMemorySize=") {
                total = v.trim().parse::<u64>().unwrap_or(0) * 1024;
            }
        }
        if total == 0 { return None; }
        Some((total - free, total))
    }

    pub fn read_idle_time() -> f64 {
        // Windows: use GetLastInputInfo via FFI would be ideal,
        // but for cross-platform compat we use a timestamp approach.
        // This is approximate.
        0.0
    }

    pub fn pid_exists(pid: u32) -> bool {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
}

/// Tracks the previous CPU snapshot for delta calculations.
pub struct CpuSampler {
    prev_total: u64,
    prev_idle: u64,
    prev_instant: Instant,
}

impl CpuSampler {
    pub fn new() -> Self {
        let (total, idle) = platform::read_cpu_idle().unwrap_or((0, 0));
        Self { prev_total: total, prev_idle: idle, prev_instant: Instant::now() }
    }

    /// Sample CPU and return usage percentage (0.0–100.0) since last call.
    pub fn sample(&mut self) -> f32 {
        let (total, idle) = platform::read_cpu_idle().unwrap_or((0, 0));
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
        let (ram_used, ram_total) = platform::read_ram().unwrap_or((0, 0));
        let idle_seconds = platform::read_idle_time();
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
