//! Glow-Up System — Intelligent Resource Allocation Policy
//!
//! The "brain" of polygoned. Translates tier + safety + real-time state
//! into concrete allocations. Lightweight, deterministic, no ML.

use crate::allocator::Allocation;
use crate::resources::{
    CpuAffinityMode, BandwidthInfo, NetInterface,
};
use crate::system::SystemSnapshot;
use crate::Platform;
use std::collections::VecDeque;
use anyhow::Result;
use serde::{Deserialize, Serialize};

// ============================================================================
// Policy types — self-contained here, re-exported via lib.rs for embedders
// ============================================================================

/// Allocation tiers — user-facing presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum AllocationTier {
    Eco,
    #[default]
    Balanced,
    Performance,
    Max,
    Custom,
}

impl AllocationTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            AllocationTier::Eco => "eco",
            AllocationTier::Balanced => "balanced",
            AllocationTier::Performance => "performance",
            AllocationTier::Max => "max",
            AllocationTier::Custom => "custom",
        }
    }
}

impl std::fmt::Display for AllocationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AllocationTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "eco" => Ok(AllocationTier::Eco),
            "balanced" => Ok(AllocationTier::Balanced),
            "performance" => Ok(AllocationTier::Performance),
            "max" => Ok(AllocationTier::Max),
            "custom" => Ok(AllocationTier::Custom),
            _ => Err(format!("unknown tier: {}", s)),
        }
    }
}

/// Resource limits within a tier (max utilisation percentages)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_cpu_percent: u8,
    pub max_ram_percent: u8,
    pub max_bandwidth_percent: u8,
    pub max_gpu_percent: u8,
}

/// Safety floors — minimum free resources the system must retain
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SafetyMargins {
    pub min_free_ram_gb: f32,
    pub min_free_cpu_cores: usize,
    pub min_free_vram_mb: u32,
    pub max_cpu_percent: u8,
}

/// A named tier preset
#[derive(Debug, Clone, Copy)]
pub struct TierPreset {
    pub name: &'static str,
    pub cpu_pct: u8,
    pub ram_pct: u8,
    pub bandwidth_pct: u8,
    pub gpu_pct: u8,
}

/// All four named presets
pub const TIER_PRESETS: [(&'static str, TierPreset); 4] = [
    ("eco",        TierPreset { name: "eco",        cpu_pct: 25, ram_pct: 30, bandwidth_pct: 40, gpu_pct: 20 }),
    ("balanced",   TierPreset { name: "balanced",   cpu_pct: 50, ram_pct: 50, bandwidth_pct: 60, gpu_pct: 40 }),
    ("performance",TierPreset { name: "performance",cpu_pct: 75, ram_pct: 70, bandwidth_pct: 80, gpu_pct: 60 }),
    ("max",        TierPreset { name: "max",        cpu_pct: 90, ram_pct: 85, bandwidth_pct: 95, gpu_pct: 80 }),
];

/// Behavioural tuning — hysteresis, step sizes, user activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub grow_step_pct: u8,
    pub shrink_step_pct: u8,
    pub shrink_hysteresis_ticks: u32,
    pub throttle_on_user_activity: bool,
    pub tick_interval_secs: u64,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        BehaviorConfig {
            grow_step_pct: 10,
            shrink_step_pct: 5,
            shrink_hysteresis_ticks: 5,
            throttle_on_user_activity: true,
            tick_interval_secs: 5,
        }
    }
}

/// Daemon configuration — tier, limits, safety, behaviour
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub tier: AllocationTier,
    pub custom_limits: Option<ResourceLimits>,
    pub behavior: BehaviorConfig,
    pub safety: SafetyMargins,
    /// Platform-specific feature toggles
    pub cpu_affinity_mode: CpuAffinityMode,
    pub memory_limit_enabled: bool,
    pub bandwidth_shaping: bool,
    pub gpu_allocation_enabled: bool,
    pub service_integration: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        DaemonConfig {
            tier: AllocationTier::Balanced,
            custom_limits: None,
            behavior: BehaviorConfig::default(),
            safety: SafetyMargins {
                min_free_ram_gb: 4.0,
                min_free_cpu_cores: 1,
                min_free_vram_mb: 512,
                max_cpu_percent: 85,
            },
            cpu_affinity_mode: CpuAffinityMode::Auto,
            memory_limit_enabled: true,
            bandwidth_shaping: true,
            gpu_allocation_enabled: true,
            service_integration: true,
        }
    }
}

impl DaemonConfig {
    /// Resolve effective limits (custom overrides tier defaults)
    pub fn effective_limits(&self, _snap: &SystemSnapshot) -> ResourceLimits {
        self.custom_limits.unwrap_or_else(|| {
            let tier_str = self.tier.to_string().to_lowercase();
            TIER_PRESETS
                .iter()
                .find(|(k, _)| *k == tier_str)
                .map(|(_, p)| ResourceLimits {
                    max_cpu_percent: p.cpu_pct,
                    max_ram_percent: p.ram_pct,
                    max_bandwidth_percent: p.bandwidth_pct,
                    max_gpu_percent: p.gpu_pct,
                })
                .unwrap_or_else(|| ResourceLimits {
                    max_cpu_percent: 50,
                    max_ram_percent: 50,
                    max_bandwidth_percent: 60,
                    max_gpu_percent: 40,
                })
        })
    }

    /// Apply safety floors to limits given current system state
    pub fn apply_safety(&self, limits: ResourceLimits, _snap: &SystemSnapshot) -> ResourceLimits {
        let mut out = limits;
        if out.max_cpu_percent > self.safety.max_cpu_percent {
            out.max_cpu_percent = self.safety.max_cpu_percent;
        }
        out
    }
}

// ============================================================================
// Glow-Up allocation engine
// ============================================================================

/// Glow-Up allocation engine
pub struct GlowUpEngine {
    pub config: DaemonConfig,
    pub current: Allocation,
    pub history: AllocationHistory,
    pub platform: Box<dyn Platform>,
}

#[derive(Debug, Clone)]
pub struct AllocationHistory {
    /// RAM allocations in bytes
    pub ram_allocations: VecDeque<u64>,
    /// CPU core allocations
    #[allow(dead_code)]
    pub cpu_allocations: VecDeque<usize>,
    /// Bandwidth allocations in Mbps
    #[allow(dead_code)]
    pub bw_allocations: VecDeque<u32>,
    /// GPU allocations in MB
    #[allow(dead_code)]
    pub gpu_allocations: VecDeque<u32>,
    max_len: usize,
}

impl AllocationHistory {
    fn new(max_len: usize) -> Self {
        Self {
            ram_allocations: VecDeque::with_capacity(max_len),
            cpu_allocations: VecDeque::with_capacity(max_len),
            bw_allocations: VecDeque::with_capacity(max_len),
            gpu_allocations: VecDeque::with_capacity(max_len),
            max_len,
        }
    }

    fn push(&mut self, alloc: &Allocation) {
        self.ram_allocations.push_back(alloc.ram_bytes);
        if self.ram_allocations.len() > self.max_len {
            self.ram_allocations.pop_front();
        }
    }

    fn trend_ram(&self) -> i64 {
        if self.ram_allocations.len() < 2 { return 0; }
        let first = *self.ram_allocations.front().unwrap() as i64;
        let last = *self.ram_allocations.back().unwrap() as i64;
        last - first
    }

    fn len(&self) -> usize {
        self.ram_allocations.len()
    }
}

impl GlowUpEngine {
    pub fn new(config: DaemonConfig, platform: Box<dyn Platform>) -> Self {
        let tier = config.tier;
        let preset = TIER_PRESETS
            .iter()
            .find(|(k, _)| *k == tier.to_string().to_lowercase())
            .map(|(_, p)| *p)
            .unwrap_or(TIER_PRESETS[1].1);

        let initial = Allocation {
            ram_bytes: (preset.ram_pct as u64 * 1024 * 1024 * 1024 / 100).min(2 * 1024 * 1024 * 1024),
            bandwidth_mbps: 10,
            shrink_streak: 0,
            free_mem_avg_bytes: 0,
            shrinking: false,
        };

        Self { config, current: initial, history: AllocationHistory::new(100), platform }
    }

    /// Main tick: compute new allocation from system snapshot
    pub fn tick(&mut self, snap: &SystemSnapshot) -> Result<Allocation> {
        let limits = self.config.effective_limits(snap);
        let safe_limits = self.config.apply_safety(limits, snap);
        let ideal = self.compute_ideal(snap, &safe_limits);
        let constrained = self.apply_constraints(ideal, snap);
        let smoothed = self.smooth_transition(constrained);

        self.current = smoothed.clone();
        self.history.push(&smoothed);
        Ok(smoothed)
    }

    fn compute_ideal(&self, snap: &SystemSnapshot, limits: &ResourceLimits) -> Allocation {
        let total_ram = snap.memory.total_bytes;
        let bw_estimate = snap.bandwidth.total_mbps.max(10.0) as u32;
        let total_vram: u32 = snap.gpu.iter().map(|g| g.vram_total_mb).sum();

        let ram_bytes = (total_ram as f32 * limits.max_ram_percent as f32 / 100.0) as u64;
        let bandwidth_mbps = (bw_estimate as f32 * limits.max_bandwidth_percent as f32 / 100.0) as u32;

        Allocation {
            ram_bytes,
            bandwidth_mbps,
            shrink_streak: 0,
            free_mem_avg_bytes: snap.memory.available_bytes,
            shrinking: false,
        }
    }

    fn apply_constraints(&self, mut alloc: Allocation, snap: &SystemSnapshot) -> Allocation {
        let behavior = &self.config.behavior;
        let safety = &self.config.safety;

        // CPU ceiling
        if snap.cpu.usage_percent > safety.max_cpu_percent as f32 {
            let factor = (safety.max_cpu_percent as f32 / snap.cpu.usage_percent).min(1.0);
            alloc.ram_bytes = (alloc.ram_bytes as f32 * factor) as u64;
            alloc.bandwidth_mbps = (alloc.bandwidth_mbps as f32 * factor) as u32;
        }

        // User activity throttle
        if behavior.throttle_on_user_activity && snap.user_active {
            alloc.ram_bytes = (alloc.ram_bytes as f32 * 0.5) as u64;
            alloc.bandwidth_mbps = (alloc.bandwidth_mbps as f32 * 0.5) as u32;
        }

        // Hard safety floors
        let min_ram = (safety.min_free_ram_gb * 1_073_741_824.0) as u64;
        if snap.memory.available_bytes < min_ram + alloc.ram_bytes {
            alloc.ram_bytes = snap.memory.available_bytes.saturating_sub(min_ram);
        }

        // Minimum floor
        alloc.ram_bytes = alloc.ram_bytes.max(64 * 1024 * 1024);
        alloc.bandwidth_mbps = alloc.bandwidth_mbps.max(1);
        alloc
    }

    fn smooth_transition(&self, target: Allocation) -> Allocation {
        let behavior = &self.config.behavior;
        let mut result = target.clone();
        let current = &self.current;

        // RAM: grow fast, shrink slow + hysteresis
        if target.ram_bytes > current.ram_bytes {
            let step = (current.ram_bytes * behavior.grow_step_pct as u64 / 100).max(64 * 1024 * 1024);
            result.ram_bytes = (current.ram_bytes + step).min(target.ram_bytes);
        } else if target.ram_bytes < current.ram_bytes {
            if self.history.trend_ram() < 0 && self.history.len() >= behavior.shrink_hysteresis_ticks as usize {
                let step = (current.ram_bytes * behavior.shrink_step_pct as u64 / 100).max(64 * 1024 * 1024);
                result.ram_bytes = current.ram_bytes.saturating_sub(step).max(target.ram_bytes);
            } else {
                result.ram_bytes = current.ram_bytes; // Hold
            }
        }

        // Bandwidth
        if target.bandwidth_mbps > current.bandwidth_mbps {
            let step = ((current.bandwidth_mbps as f32 * behavior.grow_step_pct as f32 / 100.0) as u32).max(5);
            result.bandwidth_mbps = (current.bandwidth_mbps + step).min(target.bandwidth_mbps);
        } else if target.bandwidth_mbps < current.bandwidth_mbps {
            if self.history.len() >= behavior.shrink_hysteresis_ticks as usize {
                let step = ((current.bandwidth_mbps as f32 * behavior.shrink_step_pct as f32 / 100.0) as u32).max(5);
                result.bandwidth_mbps = current.bandwidth_mbps.saturating_sub(step).max(target.bandwidth_mbps);
            } else {
                result.bandwidth_mbps = current.bandwidth_mbps;
            }
        }

        result
    }

    /// Apply allocation to platform (CPU affinity, priority, memory limit)
    pub fn apply(&self, alloc: &Allocation) -> Result<()> {
        // CPU priority via nice value
        let nice = if alloc.ram_bytes > 512 * 1024 * 1024 { 0 } else { 5 };
        self.platform.set_cpu_priority(nice)?;
        // Memory limit (best-effort)
        let _ = self.platform.set_memory_limit(alloc.ram_bytes);
        Ok(())
    }

    pub fn current(&self) -> &Allocation {
        &self.current
    }

    pub fn set_tier(&mut self, tier: AllocationTier) {
        self.config.tier = tier;
        self.config.custom_limits = None;
    }

    pub fn set_custom_limits(&mut self, limits: ResourceLimits) {
        self.config.tier = AllocationTier::Custom;
        self.config.custom_limits = Some(limits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CpuTopology;

    #[test]
    fn test_tier_presets() {
        assert_eq!(TIER_PRESETS[0].0, "eco");
        assert_eq!(TIER_PRESETS[1].0, "balanced");
        assert_eq!(TIER_PRESETS[2].0, "performance");
        assert_eq!(TIER_PRESETS[3].0, "max");
    }

    #[test]
    fn test_safety_application() {
        let config = DaemonConfig::default();
        let snap = SystemSnapshot {
            cpu: crate::CpuSnapshot {
                usage_percent: 90.0,
                per_core: vec![90.0; 8],
                load_average: [1.0, 1.0, 1.0],
                frequency_mhz: 3000.0,
                topology: CpuTopology { sockets: 1, cores_per_socket: 8, threads_per_core: 1 },
            },
            memory: crate::MemorySnapshot {
                total_bytes: 16 * 1024 * 1024 * 1024,
                used_bytes: 8 * 1024 * 1024 * 1024,
                free_bytes: 8 * 1024 * 1024 * 1024,
                available_bytes: 8 * 1024 * 1024 * 1024,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
            },
            bandwidth: crate::BandwidthSnapshot {
                interface: "eth0".into(),
                rx_mbps: 10.0,
                tx_mbps: 10.0,
                total_mbps: 20.0,
            },
            gpu: vec![crate::GpuSnapshot {
                device_id: 0,
                name: "RTX".into(),
                vram_total_mb: 8192,
                vram_used_mb: 2048,
                vram_free_mb: 6144,
                utilization_pct: 10,
                temperature_c: 40,
                power_watts: 50,
            }],
            timestamp: 0,
            user_active: false,
        };
        let limits = ResourceLimits {
            max_cpu_percent: 60,
            max_ram_percent: 45,
            max_bandwidth_percent: 70,
            max_gpu_percent: 40,
        };
        let safe = config.apply_safety(limits, &snap);
        assert!(safe.max_cpu_percent <= 85);
    }
}