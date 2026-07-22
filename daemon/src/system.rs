//! Core system information collector.
//!
//! Wozniak: know your hardware, use what's available, no surprises.
//!
//! Defines `SystemSnapshot` and its sub-structs — the canonical system-state
//! type consumed by the GlowUp allocation engine.

use crate::resources::CpuTopology;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Mutex;

// ============================================================================
// Snapshot types — these match what glow_up.rs and main.rs expect
// ============================================================================

/// CPU snapshot for a single tick.
#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    /// Overall CPU utilisation 0–100.
    pub usage_percent: f32,
    /// Per-core frequency in MHz (may be empty on some platforms).
    pub per_core: Vec<f32>,
    /// 1-minute load average.
    pub load_average: [f32; 3],
    /// Average frequency across cores in MHz.
    pub frequency_mhz: f32,
    /// CPU topology.
    pub topology: CpuTopology,
}

/// Memory snapshot for a single tick.
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
}

/// Bandwidth snapshot for a single tick.
#[derive(Debug, Clone)]
pub struct BandwidthSnapshot {
    pub interface: String,
    pub rx_mbps: f64,
    pub tx_mbps: f64,
    pub total_mbps: f64,
}

/// GPU snapshot for a single tick.
#[derive(Debug, Clone)]
pub struct GpuSnapshot {
    pub device_id: u32,
    pub name: String,
    pub vram_total_mb: u32,
    pub vram_used_mb: u32,
    pub vram_free_mb: u32,
    pub utilization_pct: u32,
    pub temperature_c: u32,
    pub power_watts: u32,
}

/// Complete system snapshot — the input to the GlowUp tick() function.
#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub bandwidth: BandwidthSnapshot,
    pub gpu: Vec<GpuSnapshot>,
    pub timestamp: i64,
    /// Whether a human is currently using the machine.
    pub user_active: bool,
}

impl SystemSnapshot {
    /// Capture a new snapshot from live system data.
    pub fn capture(platform: &dyn crate::Platform) -> Self {
        let cpu_info = platform.cpu_info().unwrap_or_else(|_| crate::CpuInfo {
            cores: 1,
            model: String::new(),
            topology: crate::CpuTopology::default(),
            per_core: vec![],
        });
        let mem_info = platform.memory_info().unwrap_or_else(|_| crate::MemoryInfo::default());
        let bw_info = platform.bandwidth_info().unwrap_or_else(|_| crate::BandwidthInfo::default());
        let gpus = platform.gpu_info().unwrap_or_default();
        let user_active = platform.user_active().unwrap_or(false);

        let primary_iface = bw_info.interfaces.iter()
            .find(|i| !i.is_loopback && i.is_up)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let (rx_mbps, tx_mbps) = bw_info.primary_rx_tx();
        let total_mbps = rx_mbps + tx_mbps;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            cpu: CpuSnapshot {
                usage_percent: sysinfo_cpu_usage(),
                per_core: cpu_info.per_core.iter().map(|x| *x as f32).collect(),
                load_average: [0.0; 3],
                frequency_mhz: {
                    let sum: f32 = cpu_info.per_core.iter().map(|x| *x as f32).sum();
                    sum / cpu_info.per_core.len().max(1) as f32
                },
                topology: cpu_info.topology,
            },
            memory: MemorySnapshot {
                total_bytes: mem_info.total_bytes,
                used_bytes: mem_info.used_bytes,
                free_bytes: mem_info.free_bytes,
                available_bytes: mem_info.available_bytes,
                swap_used_bytes: mem_info.swap_total_bytes.saturating_sub(mem_info.swap_free_bytes),
                swap_total_bytes: mem_info.swap_total_bytes,
            },
            bandwidth: BandwidthSnapshot {
                interface: primary_iface,
                rx_mbps: (rx_mbps as f64) / 1_000_000.0 * 8.0,
                tx_mbps: (tx_mbps as f64) / 1_000_000.0 * 8.0,
                total_mbps: (rx_mbps + tx_mbps) as f64 * 8.0 / 1_000_000.0,
            },
            gpu: gpus.into_iter().map(|g| GpuSnapshot {
                device_id: g.device_id,
                name: g.name,
                vram_total_mb: g.total_vram_mb,
                vram_used_mb: g.used_vram_mb,
                vram_free_mb: g.free_vram_mb,
                utilization_pct: g.utilization_pct,
                temperature_c: g.temperature_c,
                power_watts: g.power_watts,
            }).collect(),
            timestamp: timestamp,
            user_active: user_active,
        }
    }
}

// ============================================================================
// sysinfo helpers (used internally, pre-date the snapshot types)
// ============================================================================

static SYSTEM: Mutex<Option<System>> = Mutex::new(None);

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

pub fn total_ram_bytes() -> u64 {
    with_sys(|s| s.total_memory())
}

pub fn used_ram_bytes() -> u64 {
    with_sys(|s| s.used_memory())
}

pub fn free_ram_bytes() -> u64 {
    with_sys(|s| s.available_memory())
}

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

fn sysinfo_cpu_usage() -> f32 {
    refresh();
    cpu_usage_percent()
}

/// True if user is actively using this machine.
/// Heuristic: if any CPU core > 50%, assume active user.
pub fn user_is_active() -> bool {
    with_sys(|s| s.cpus().iter().any(|c| c.cpu_usage() > 50.0))
}

// ============================================================================
// Legacy flat SystemSnapshot — kept for allocator.rs compatibility
// ============================================================================

/// Legacy flat snapshot — still used by the old `allocator` module.
#[derive(Debug, Clone)]
pub struct LegacySystemSnapshot {
    pub total_ram_gb: f64,
    pub used_ram_gb: f64,
    pub free_ram_gb: f64,
    pub cpu_cores: usize,
    pub cpu_usage_pct: f32,
    pub user_active: bool,
}

impl LegacySystemSnapshot {
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

// re-export for use in tests below
pub use crate::allocator::Allocation;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_snapshot_capture() {
        let s = LegacySystemSnapshot::capture();
        assert!(s.total_ram_gb > 0.0);
        assert!(s.free_ram_gb >= 0.0);
        assert!(s.cpu_cores >= 1);
    }
}