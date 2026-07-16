//! Core system information collector.
//! Wozniak: know your hardware, use what's available, no surprises.

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use std::sync::Mutex;

/// We wrap System in a Mutex because sysinfo 0.32 requires `&mut self`
/// for refresh methods. This keeps the API ergonomic for callers.
static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

/// Access the System handle under the Mutex.
/// Callers must pass a closure that operates on `&System`.
pub fn with_sys<R>(f: impl FnOnce(&System) -> R) -> R {
    let mut guard = SYSTEM.lock().expect("SYSTEM mutex poisoned");
    if guard.is_none() {
        *guard = Some(System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        ));
    }
    f(guard.as_ref().expect("sysinfo initialized"))
}

/// Refresh system state (call this every tick, not continuously).
pub fn refresh() {
    let mut guard = SYSTEM.lock().expect("SYSTEM mutex poisoned");
    if guard.is_none() {
        *guard = Some(System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        ));
    }
    if let Some(s) = guard.as_mut() {
        s.refresh_cpu_all();
        s.refresh_memory();
    }
}

/// Total RAM in bytes.
pub fn total_ram_bytes() -> u64 {
    with_sys(|s| s.total_memory())
}

/// Currently used RAM in bytes.
pub fn used_ram_bytes() -> u64 {
    with_sys(|s| s.used_memory())
}

/// Free RAM in bytes (what's not in use by anyone).
pub fn free_ram_bytes() -> u64 {
    with_sys(|s| s.available_memory())
}

/// Total CPU cores (physical).
pub fn cpu_cores() -> usize {
    with_sys(|s| s.cpus().len())
}

/// Average CPU usage across all cores (0.0–100.0).
pub fn cpu_usage_percent() -> f32 {
    with_sys(|s| {
        let cores = s.cpus().len();
        if cores == 0 { return 0.0; }
        s.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cores as f32
    })
}

/// True if user is actively using this machine.
/// Heuristic: if any CPU core > 50%, assume active user.
pub fn user_is_active() -> bool {
    with_sys(|s| s.cpus().iter().any(|c| c.cpu_usage() > 50.0))
}

/// Human-readable snapshot of system state.
#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub total_ram_gb: f64,
    pub used_ram_gb: f64,
    pub free_ram_gb: f64,
    pub cpu_cores: usize,
    pub cpu_usage_pct: f32,
    pub user_active: bool,
}

impl SystemSnapshot {
    pub fn capture() -> Self {
        refresh();
        Self {
            total_ram_gb: total_ram_bytes() as f64 / 1_073_741_824.0,
            used_ram_gb: used_ram_bytes() as f64 / 1_073_741_824.0,
            free_ram_gb: free_ram_bytes() as f64 / 1_073_741_824.0,
            cpu_cores: cpu_cores(),
            cpu_usage_pct: cpu_usage_percent(),
            user_active: user_is_active(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_capture() {
        let s = SystemSnapshot::capture();
        assert!(s.total_ram_gb > 0.0);
        assert!(s.free_ram_gb >= 0.0);
        assert!(s.cpu_cores >= 1);
    }
}