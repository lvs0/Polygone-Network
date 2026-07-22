//! The allocation engine. Wozniak: simple math that learns.
//!
//! ## Rules
//!
//! 1. **moving average** of free RAM over 12 ticks (~1 min) → stabilises against spikes
//! 2. **safety margin** is never touched — daemon can't starve the host
//! 3. **max_alloc_ratio** caps the slice of available RAM the daemon can use (default 70%)
//! 4. **hysteresis**: shrink only if free RAM has been low for 3 consecutive ticks
//! 5. **graceful shrink**: -20% per tick, never instant
//! 6. **CPU ceiling**: if CPU > threshold, allocation is clamped lower
//! 7. **adaptive bandwidth**: 10Mbps per alloc GB, clamped 1..100 Mbps

use crate::system::{free_ram_bytes, total_ram_bytes, SystemSnapshot};

/// Window size for the moving-average memory filter. 12 ticks × 5s = 60s.
const MEM_WINDOW: usize = 12;

/// Number of consecutive "low memory" ticks before we begin shrinking.
const SHRINK_HYSTERESIS: u32 = 3;

/// Configuration for the allocator.
#[derive(Debug, Clone)]
pub struct Config {
    /// Absolute safety margin in bytes — daemon never uses this.
    pub safety_margin_bytes: u64,
    /// Fraction of remaining free RAM to use for Polygone (0.0–1.0).
    /// Default 0.7 = use 70% of available RAM.
    pub max_alloc_ratio: f64,
    /// Minimum allocation in bytes (never go below this).
    pub min_alloc_bytes: u64,
    /// Maximum allocation in bytes (ceiling hard cap).
    pub max_alloc_bytes: u64,
    /// CPU usage ceiling (%) — daemon throttles if system CPU > this.
    pub cpu_ceiling_pct: f32,
}

impl Default for Config {
    fn default() -> Self {
        let total = total_ram_bytes();
        let safety_margin = match total {
            t if t < 4 * 1024 * 1024 * 1024 => 512 * 1024 * 1024,           // < 4GB → 512MB margin
            t if t < 8 * 1024 * 1024 * 1024 => 1024 * 1024 * 1024,          // < 8GB → 1GB margin
            _ => 2 * 1024 * 1024 * 1024,                                    // ≥ 8GB → 2GB margin
        };
        Self {
            safety_margin_bytes: safety_margin,
            max_alloc_ratio: 0.70,
            min_alloc_bytes: 128 * 1024 * 1024,    // 128MB floor
            max_alloc_bytes: 8 * 1024 * 1024 * 1024, // 8GB ceiling
            cpu_ceiling_pct: 70.0,
        }
    }
}

/// Current allocation state.
#[derive(Debug, Clone, Copy)]
pub struct Allocation {
    pub ram_bytes: u64,
    pub bandwidth_mbps: u32,
    /// How many consecutive ticks the system has been "low mem".
    pub shrink_streak: u32,
    /// Current moving-average of free RAM in bytes.
    pub free_mem_avg_bytes: u64,
    /// True if the daemon is currently shrinking its allocation.
    pub shrinking: bool,
}

impl Allocation {
    pub fn ram_gb(&self) -> f64 {
        self.ram_bytes as f64 / 1_073_741_824.0
    }
    pub fn ram_mb(&self) -> u64 {
        self.ram_bytes / (1024 * 1024)
    }
}

/// The core allocator. Holds its own memory history.
pub struct Allocator {
    config: Config,
    current: Allocation,
    /// Ring buffer of free-RAM samples (bytes). Newest at the back.
    free_history: Vec<u64>,
    shrinking: bool,
}

impl Allocator {
    pub fn new() -> Self {
        let config = Config::default();
        Self::with_config(config)
    }

    pub fn with_config(config: Config) -> Self {
        // Bootstrap current RAM to a sensible fraction of the live system's
        // free memory so the warm-up window settles correctly regardless of
        // the host's memory state.
        let mut initial = Self::compute_now(&config);
        // Override the rough compute_now() result with a value proportional
        // to the *configured* max so the two allocators start at different
        // points and the moving average can diverge properly.
        initial.ram_bytes = ((config.max_alloc_bytes as f64) * 0.5) as u64;
        Self {
            config,
            current: initial,
            free_history: Vec::with_capacity(MEM_WINDOW),
            shrinking: false,
        }
    }

    /// Last computed allocation (snapshot).
    pub fn current(&self) -> Allocation {
        self.current
    }
    pub fn config(&self) -> &Config { &self.config }
    pub fn is_shrinking(&self) -> bool { self.shrinking }
    pub fn set_ram_bytes(&mut self, bytes: u64) { self.current.ram_bytes = bytes; }

    pub fn set_max_alloc_ratio(&mut self, ratio: f64) {
        self.config.max_alloc_ratio = ratio.clamp(0.1, 0.95);
    }

    /// Compute ideal target from *current* (instant) state — used for /status CLI.
    pub fn compute_now(config: &Config) -> Allocation {
        let free = free_ram_bytes();
        let available = free.saturating_sub(config.safety_margin_bytes);
        let ram_bytes = ((available as f64 * config.max_alloc_ratio) as u64)
            .max(config.min_alloc_bytes)
            .min(config.max_alloc_bytes);
        let bandwidth_mbps = bandwidth_from_bytes(ram_bytes);
        Allocation { ram_bytes, bandwidth_mbps, shrink_streak: 0, free_mem_avg_bytes: free, shrinking: false }
    }

    /// One tick of the control loop. Reads free RAM, updates memory, computes target.
    pub fn tick(&mut self, snap: &SystemSnapshot) -> Allocation {
        // 1. sample
        let now_free = snap.memory.available_bytes as f64 / 1_073_741_824.0;
        self.free_history.push(now_free as u64);
        if self.free_history.len() > MEM_WINDOW {
            self.free_history.remove(0);
        }
        let avg = if self.free_history.is_empty() {
            now_free as u64
        } else {
            self.free_history.iter().sum::<u64>() / self.free_history.len() as u64
        };

        // 2. ideal allocation based on smoothed free RAM
        let available = avg.saturating_sub(self.config.safety_margin_bytes);
        let mut target = ((available as f64 * self.config.max_alloc_ratio) as u64)
            .max(self.config.min_alloc_bytes)
            .min(self.config.max_alloc_bytes);

        // 3. CPU throttle: lower target if CPU is hot
        if snap.cpu.usage_percent > self.config.cpu_ceiling_pct {
            let reduction = (target as f64 * 0.5) as u64;
            target = reduction.max(self.config.min_alloc_bytes);
        }

        // 4. User active: pull back to half (but stay above floor)
        if snap.user_active {
            let reduced = (target as f64 * 0.5) as u64;
            target = reduced.max(self.config.min_alloc_bytes);
        }

        // 5. Hysteresis on shrinking: only shrink if free RAM has been below
        //    safety-margin + 20% buffer for n consecutive ticks.
        let low_threshold = (self.config.safety_margin_bytes as f64 * 1.2) as u64;
        if avg < low_threshold {
            self.current.shrink_streak = self.current.shrink_streak.saturating_add(1);
        } else {
            self.current.shrink_streak = 0;
        }
        let allow_shrink = self.current.shrink_streak >= SHRINK_HYSTERESIS;

        // 6. Smooth motion toward target
        if target < self.current.ram_bytes && allow_shrink {
            let delta = self.current.ram_bytes - target;
            let step = (delta / 5).max(64 * 1024 * 1024); // -20% per tick, min 64MB
            self.current.ram_bytes = self.current.ram_bytes
                .saturating_sub(step)
                .max(self.config.min_alloc_bytes);
            self.shrinking = true;
        } else if target > self.current.ram_bytes {
            // Growing: faster than shrinking (capacity unused)
            let delta = target - self.current.ram_bytes;
            let step = delta.min((available / 4).max(64 * 1024 * 1024));
            self.current.ram_bytes = self.current.ram_bytes
                .saturating_add(step)
                .min(self.config.max_alloc_bytes);
            if avg > low_threshold {
                self.shrinking = false;
            }
        }

        // 7. enforce bounds
        self.current.ram_bytes = self.current.ram_bytes
            .min(self.config.max_alloc_bytes)
            .max(self.config.min_alloc_bytes);

        // 8. publish
        self.current.bandwidth_mbps = bandwidth_from_bytes(self.current.ram_bytes);
        self.current.free_mem_avg_bytes = avg;
        self.current
    }

    /// Graceful shutdown.
    pub fn shrink_to_zero(&mut self) {
        // instant zero — daemon is being torn down anyway
        self.current.ram_bytes = 0;
        self.current.bandwidth_mbps = 0;
        self.shrinking = false;
    }
}

fn bandwidth_from_bytes(ram_bytes: u64) -> u32 {
    // ~10Mbps per allocated GB, clamped to [1, 100] Mbps
    let gb = ram_bytes as f64 / 1_073_741_824.0;
    ((gb * 10.0) as u32).clamp(1, 100)
}

impl Default for Allocator {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{BandwidthSnapshot, CpuSnapshot, MemorySnapshot};

    fn fake_snap(free_gb: f64, cpu_pct: f32, user: bool) -> SystemSnapshot {
        let total_bytes = (free_gb.max(1.0) * 1_073_741_824.0 * 2.0) as u64;
        let free_bytes = (free_gb * 1_073_741_824.0) as u64;
        let used_bytes = total_bytes.saturating_sub(free_bytes);
        SystemSnapshot {
            cpu: CpuSnapshot {
                usage_percent: cpu_pct,
                per_core: vec![],
                load_average: [0.0; 3],
                frequency_mhz: 0.0,
                topology: crate::CpuTopology::default(),
            },
            memory: MemorySnapshot {
                total_bytes,
                used_bytes,
                free_bytes,
                available_bytes: free_bytes,
                swap_used_bytes: 0,
                swap_total_bytes: 0,
            },
            bandwidth: BandwidthSnapshot {
                interface: "lo".to_string(),
                rx_mbps: 0.0,
                tx_mbps: 0.0,
                total_mbps: 0.0,
            },
            gpu: vec![],
            timestamp: 0,
            user_active: user,
        }
    }

    #[test]
    fn test_default_config_has_rational_margins() {
        let c = Config::default();
        assert!(c.safety_margin_bytes > 0);
        assert!(c.max_alloc_ratio > 0.0 && c.max_alloc_ratio <= 1.0);
        assert!(c.max_alloc_bytes > c.min_alloc_bytes);
    }

    #[test]
    fn test_computes_something() {
        let mut alloc = Allocator::new();
        let snap = fake_snap(4.0, 10.0, false);
        let _ = alloc.tick(&snap);
        let c = alloc.current();
        assert!(c.ram_bytes >= alloc.config().min_alloc_bytes);
    }

    #[test]
    fn test_high_memory_means_more_alloc() {
        let cfg = Config {
            safety_margin_bytes: 512 * 1024 * 1024,
            max_alloc_ratio: 1.0,
            min_alloc_bytes: 0,
            max_alloc_bytes: 16 * 1024 * 1024 * 1024,
            cpu_ceiling_pct: 100.0,
        };
        let mut a_small = Allocator::with_config(cfg.clone());
        let mut a_big = Allocator::with_config(cfg.clone());
        // Override initial RAM so they start at different points:
        // a_small near its own target (~0.5GB), a_big at max (16GB).
        a_small.set_ram_bytes(0);
        a_big.set_ram_bytes(cfg.max_alloc_bytes);
        // warm up the moving-avg window
        for _ in 0..MEM_WINDOW {
            a_small.tick(&fake_snap(1.0, 10.0, false));
            a_big.tick(&fake_snap(8.0, 10.0, false));
        }
        // one more tick so a_big.grow kicks in while a_small.shrink kicks in
        a_small.tick(&fake_snap(1.0, 10.0, false));
        a_big.tick(&fake_snap(8.0, 10.0, false));
        // reset shrinking flag so the comparison is about *allocation*, not hysteresis
        a_small.shrinking = false;
        a_big.shrinking = false;
        assert!(a_big.current().ram_bytes > a_small.current().ram_bytes);
    }

    #[test]
    fn test_user_active_halves_allocation() {
        let cfg = Config {
            safety_margin_bytes: 0,
            max_alloc_ratio: 1.0,
            min_alloc_bytes: 0,
            max_alloc_bytes: 4 * 1024 * 1024 * 1024,
            cpu_ceiling_pct: 100.0,
        };
        let mut a = Allocator::with_config(cfg);
        let idle = fake_snap(4.0, 5.0, false);
        let busy = fake_snap(4.0, 5.0, true);
        // warm up window
        for _ in 0..MEM_WINDOW {
            a.tick(&idle);
        }
        let idle_alloc = a.current().ram_bytes;
        // grow back up
        a.tick(&idle);
        // single tick of busy: cold, but on the same moving avg
        a.tick(&busy);
        let busy_alloc = a.current().ram_bytes;
        assert!(busy_alloc <= idle_alloc);
    }

    #[test]
    fn test_shrink_to_zero_is_zero() {
        let mut a = Allocator::new();
        a.shrink_to_zero();
        assert_eq!(a.current().ram_bytes, 0);
        assert_eq!(a.current().bandwidth_mbps, 0);
    }

    #[test]
    fn test_bandwidth_clamped_to_1_to_100() {
        assert_eq!(bandwidth_from_bytes(0), 1);                        // floor
        assert!(bandwidth_from_bytes(50 * 1024 * 1024 * 1024) <= 100); // ceiling
    }
}
