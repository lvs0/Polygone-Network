//! Bandwidth monitoring and allocation.
//!
//! ## What this does
//!
//! - **detect** active interfaces via `/sys/class/net/`
//! - **measure** real rx/tx bytes from `statistics/rx_bytes` and `statistics/tx_bytes`
//! - **compute** a sliding-window average (60 s) of bandwidth in Mbps
//! - **report** `BandwidthAllocation { mbps_actual, mbps_allocated, interface }`
//!
//! ## What this doesn't do (yet)
//!
//! - Traffic shaping (tc/iptables) — requires root and is system-wide,
//!   which collides with multi-user systems. Real shaping belongs to
//!   `polygone-client` (which has its own cgroup/namespace) or to a
//!   system-level NetworkManager hook, not to a daemon that shouldn't need root.
//!
//! - For now: bandwidth is *reported* (measured live), not enforced.

use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Window for bandwidth averaging: 12 ticks × 5 s = 60 s.
const BW_WINDOW: usize = 12;

/// A bandwidth sample: bytes transferred over a time delta.
#[derive(Debug, Clone)]
struct Sample {
    rx_bytes: u64,
    tx_bytes: u64,
    elapsed_secs: f64,
}

/// Live bandwidth allocation state.
#[derive(Debug, Clone)]
pub struct BandwidthAllocation {
    /// Measured inbound bandwidth (Mbps).
    pub rx_mbps: f64,
    /// Measured outbound bandwidth (Mbps).
    pub tx_mbps: f64,
    /// Allocated outbound bandwidth ceiling (Mbps) — our share.
    pub alloc_mbps: u32,
    /// Interface used for measurement.
    pub interface: String,
}

impl BandwidthAllocation {
    /// Total bandwidth (rx + tx) in Mbps.
    pub fn total_mbps(&self) -> f64 { self.rx_mbps + self.tx_mbps }
    /// Check if the allocation is saturated (little headroom).
    pub fn is_saturated(&self, threshold_mbps: f64) -> bool {
        self.alloc_mbps as f64 - self.total_mbps() < threshold_mbps
    }
}

/// Bandwidth monitor. Reads /sys/class/net/ on every tick.
pub struct Monitor {
    interface: String,
    history: VecDeque<Sample>,
    last_rx: u64,
    last_tx: u64,
    last_instant: Option<Instant>,
    /// Allocated share in Mbps (set by main loop based on RAM allocation).
    pub allocated_mbps: u32,
}

impl Monitor {
    /// Create a new monitor for the given interface. Picks the first
    /// non-loopback interface, or falls back to "lo" if none found.
    pub fn new(interface: Option<&str>) -> Self {
        let iface = interface
            .map(String::from)
            .or_else(|| detect_primary_interface())
            .unwrap_or_else(|| "lo".to_string());

        log::info!("bandwidth: monitoring interface {}", iface);
        let (rx, tx) = read_counters(&iface).unwrap_or((0, 0));

        Self {
            interface: iface,
            history: VecDeque::with_capacity(BW_WINDOW),
            last_rx: rx,
            last_tx: tx,
            last_instant: Some(Instant::now()),
            allocated_mbps: 0,
        }
    }

    /// Tick: measure bandwidth on this interval, update history, return allocation.
    pub fn tick(&mut self) -> BandwidthAllocation {
        let now = Instant::now();
        let (rx, tx) = read_counters(&self.interface).unwrap_or((self.last_rx, self.last_tx));

        let elapsed = self.last_instant
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(1.0)
            .max(0.001);

        // Bytes/sec → Mbps (÷ 1_000_000, then × 8 for Mb)
        let _rx_bps = (rx.saturating_sub(self.last_rx)) as f64 / elapsed;
        let _tx_bps = (tx.saturating_sub(self.last_tx)) as f64 / elapsed;

        // Store absolute counter snapshot (for delta computation next tick)
        self.history.push_back(Sample { rx_bytes: rx, tx_bytes: tx, elapsed_secs: elapsed });
        if self.history.len() > BW_WINDOW {
            self.history.pop_front();
        }

        self.last_rx = rx;
        self.last_tx = tx;
        self.last_instant = Some(now);

        // Sliding average over window (in Mbps)
        let avg = self.avg_mbps();

        BandwidthAllocation {
            rx_mbps: avg.0,
            tx_mbps: avg.1,
            alloc_mbps: self.allocated_mbps,
            interface: self.interface.clone(),
        }
    }

    /// Set allocated share (called by main loop based on RAM).
    pub fn set_allocated(&mut self, mbps: u32) {
        self.allocated_mbps = mbps;
    }

    /// Sliding average of rx/tx bandwidth in Mbps over the history window.
    /// Computes per-tick rates then averages them.
    fn avg_mbps(&self) -> (f64, f64) {
        if self.history.len() < 2 { return (0.0, 0.0); }

        let mut rx_rates: Vec<f64> = Vec::new();
        let mut tx_rates: Vec<f64> = Vec::new();
        let samples: Vec<_> = self.history.iter().collect();

        for i in 1..samples.len() {
            let prev = samples[i - 1];
            let curr = samples[i];
            let delta_rx = curr.rx_bytes.saturating_sub(prev.rx_bytes) as f64;
            let delta_tx = curr.tx_bytes.saturating_sub(prev.tx_bytes) as f64;
            let elapsed = curr.elapsed_secs.max(0.001);
            rx_rates.push(delta_rx * 8.0 / (elapsed * 1_000_000.0));
            tx_rates.push(delta_tx * 8.0 / (elapsed * 1_000_000.0));
        }

        let avg_rx = rx_rates.iter().sum::<f64>() / rx_rates.len() as f64;
        let avg_tx = tx_rates.iter().sum::<f64>() / tx_rates.len() as f64;
        (avg_rx, avg_tx)
    }

    /// Reset counters (useful after sleep/wake).
    pub fn reset(&mut self) {
        let (rx, tx) = read_counters(&self.interface).unwrap_or((0, 0));
        self.last_rx = rx;
        self.last_tx = tx;
        self.last_instant = Some(Instant::now());
        self.history.clear();
    }
}

/// Read current rx/tx byte counters for a network interface.
fn read_counters(iface: &str) -> std::io::Result<(u64, u64)> {
    let rx = fs::read_to_string(format!("/sys/class/net/{}/statistics/rx_bytes", iface))?
        .trim().parse().unwrap_or(0);
    let tx = fs::read_to_string(format!("/sys/class/net/{}/statistics/tx_bytes", iface))?
        .trim().parse().unwrap_or(0);
    Ok((rx, tx))
}

/// Pick the "primary" outbound interface (non-loopback, has traffic).
/// Strategy: read rx_bytes on each candidate and pick the one with the most traffic.
fn detect_primary_interface() -> Option<String> {
    let net = Path::new("/sys/class/net");
    let entries = fs::read_dir(net).ok()?;

    let mut candidates: Vec<(String, u64)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "lo" { continue; }
        // Check it has an address (indicating it's up)
        let addr_path = format!("/sys/class/net/{}/address", name);
        if fs::read_to_string(&addr_path).is_ok() {
            // Use rx_bytes as a proxy for "is this interface doing work"
            let rx_path = format!("/sys/class/net/{}/statistics/rx_bytes", name);
            if let Ok(s) = fs::read_to_string(&rx_path) {
                if let Ok(rx) = s.trim().parse::<u64>() {
                    candidates.push((name, rx));
                }
            }
        }
    }
    // Pick the interface with the most received traffic — most likely the active one
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    log::debug!("bandwidth: detected interfaces: {:?}", candidates);
    candidates.into_iter().next().map(|(n, _)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_creates_and_ticks() {
        let mut m = Monitor::new(Some("lo"));
        m.allocated_mbps = 10;
        let bw = m.tick();
        // lo should show some traffic (at least 0 mbps on first tick)
        assert_eq!(bw.interface, "lo");
        assert!(bw.alloc_mbps <= 100); // sanity
    }

    #[test]
    fn read_counters_on_lo() {
        let (rx, tx) = read_counters("lo").unwrap();
        assert!(rx > 0 || tx > 0, "lo should have some traffic");
    }

    #[test]
    fn detect_primary_interface_skips_loopback() {
        let iface = detect_primary_interface();
        assert!(iface.is_none() || iface.as_ref().map(|s| s != "lo").unwrap(),
            "should not return lo");
    }

    #[test]
    fn bandwidth_allocation_saturated() {
        let bw = BandwidthAllocation {
            rx_mbps: 5.0, tx_mbps: 3.0, alloc_mbps: 10, interface: "lo".into()
        };
        // 5+3=8, alloc=10, threshold=3 → 10-8=2 < 3 → saturated
        assert!(bw.is_saturated(3.0));
        // threshold=5 → 10-8=2 < 5 → saturated
        assert!(bw.is_saturated(5.0));
        // threshold=1 → 10-8=2 >= 1 → not saturated
        assert!(!bw.is_saturated(1.0));
    }
}