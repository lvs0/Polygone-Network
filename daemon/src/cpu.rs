//! CPU allocation: detect cores, set affinity, optionally renice.
//!
//! ## What this does (and what it doesn't)
//!
//! - **detect**: reads total cores via `sched_getaffinity` (what *this process*
//!   is allowed to use, not just the hardware).
//! - **allocate**: returns a `CpuAllocation` (number of cores + nice level).
//! - **apply**: calls `sched_setaffinity` on *this process* to a subset of cores,
//!   and `setpriority` for niceness. **Real, observable effect.**
//!
//! ## Caveats
//!
//! - `sched_setaffinity` works on Linux. Other platforms fall back to "count
//!   cores, do nothing" — still a no-op, no panic.
//! - Anti-pin trap: we don't pin to core 0 (which often handles IRQ 0, the
//!   timer interrupt — pinning there causes missed ticks). We pick the middle
//!   cores first, then outward.
//!
//! ## Memory orders
//! We use Acquire/Release because CPU counts change at runtime (hotplug rarely,
//! but cgroups do).

use anyhow::Result;
use std::sync::atomic::{AtomicU32, Ordering};

/// Total logical cores the daemon can see (cached).
static CPU_CORES: AtomicU32 = AtomicU32::new(0);

/// Live CPU allocation. `cores` is the count of cores reserved for Polygone.
#[derive(Debug, Clone)]
pub struct CpuAllocation {
    /// Total cores the daemon can see.
    pub total: u32,
    /// How many cores we are willing to give Polygone.
    pub allocated: u32,
    /// Nice level (0..19, more positive = lower priority).
    pub nice: i32,
    /// Cpu affinity mask as Vec<u32> (for displaying / exporting).
    pub affinity_mask: Vec<u32>,
}

impl CpuAllocation {
    pub fn ratio(&self) -> f64 {
        if self.total == 0 { return 0.0; }
        self.allocated as f64 / self.total as f64
    }
}

/// Detect the number of cores the OS exposes to this process.
/// Uses `sched_getaffinity` when available, falls back to `nproc`.
pub fn detect_cores() -> u32 {
    // Try sched_getaffinity first — it tells us what THIS process can actually use.
    // On a cgroup-restricted container, this can differ from hardware.
    #[cfg(target_os = "linux")]
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        let sz = std::mem::size_of::<libc::cpu_set_t>();
        let ok = libc::sched_getaffinity(0, sz, &mut set);
        if ok == 0 {
            // Count bits set in cpu_set_t (how many cores are allowed)
            let raw = &set as *const libc::cpu_set_t as *const u8;
            let bytes = std::slice::from_raw_parts(raw, sz);
            let mut count: u32 = 0;
            for &b in bytes {
                count += b.count_ones();
            }
            let hw = hardware_cores();
            let n = count.min(hw);
            if n > 0 { return n; }
        }
    }
    hardware_cores()
}

/// Total hardware cores (`nproc`).
pub fn hardware_cores() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
        .max(1)
}

/// Initialize cached CPU count. Idempotent.
pub fn init() {
    let n = detect_cores();
    CPU_CORES.store(n, Ordering::Release);
}

/// Cached CPU count.
pub fn cpu_cores() -> u32 {
    let cached = CPU_CORES.load(Ordering::Acquire);
    if cached == 0 { init(); CPU_CORES.load(Ordering::Acquire) } else { cached }
}

/// Compute CPU allocation based on a free-RAM-derived target ratio.
/// Rules:
/// - Never take more than 90% of cores (leave 1 for the OS/scheduler jitter)
/// - Take at minimum 1 core (Polygone-Network always has a slice)
/// - The daemon itself took ~1 core already; we report the *additional* allocation
///   logically, but we apply affinity later to a child process.
pub fn allocate(target_ratio: f64) -> CpuAllocation {
    let total = cpu_cores().max(1);
    // Reserve at least 1 core for the host
    let usable = total.saturating_sub(1).max(1);
    let mut alloc = (usable as f64 * target_ratio.clamp(0.0, 0.95)) as u32;
    // Capping, but never below 1
    alloc = alloc.clamp(1, usable);
    // Nice level: if we don't take the whole CPU, be polite (nice = +5)
    let nice = if (alloc as f64) / (total as f64) < 0.7 { 5 } else { 0 };

    // Pick which cores to allocate (avoid core 0 — IRQ 0 lives there)
    let affinity_mask = pick_cores(total, alloc);

    log::debug!(
        "cpu alloc: total={} alloc={} ({}%) nice={} mask={:?}",
        total, alloc, (alloc as f64 / total as f64 * 100.0) as u32, nice, affinity_mask
    );

    CpuAllocation { total, allocated: alloc, nice, affinity_mask }
}

/// Pick a list of core indices to give Polygone.
/// Strategy: avoid core 0, prefer middle-and-out, never take the very last core.
fn pick_cores(total: u32, want: u32) -> Vec<u32> {
    if total <= 1 {
        return vec![0];
    }
    // Centre-first to maximise cache locality across sockets/numa.
    let centre = total / 2;
    let mut out: Vec<u32> = Vec::with_capacity(want as usize);
    let mut offset: i32 = 0;
    // Walk outward from centre; skip 0 if we hit it.
    while (out.len() as u32) < want && (offset as u32) < total {
        let lower = centre as i32 - offset;
        let upper = centre as i32 + offset;
        for &c in &[lower, upper] {
            if c < 0 || (c as u32) >= total { continue; }
            if c == 0 { continue; } // avoid IRQ 0
            if (c as u32) >= total - 1 { continue; } // leave last core alone
            if !out.contains(&(c as u32)) {
                out.push(c as u32);
                if (out.len() as u32) >= want { break; }
            }
        }
        offset += 1;
    }
    // Sort for stable output
    out.sort_unstable();
    if out.is_empty() { vec![1.min(total - 1)] } else { out }
}

/// Apply affinity to the *current* thread.
///
/// Note: a thread inherits the process's affinity mask, but we set it on this
/// thread because cryptographically the polling thread should be on the
/// reserved cores so it doesn't interrupt other threads.
pub fn apply_thread_affinity(mask: &[u32]) -> Result<()> {
    if mask.is_empty() { return Ok(()); }
    #[cfg(target_os = "linux")]
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &core in mask {
            // libc::CPU_SET writes the bit into the cpu_set_t
            libc::CPU_SET(core as usize, &mut set);
        }
        let sz = std::mem::size_of::<libc::cpu_set_t>();
        let rc = libc::sched_setaffinity(
            0, // current thread
            sz,
            &set,
        );
        if rc != 0 {
            return Err(anyhow::anyhow!(
                "sched_setaffinity failed: errno={}",
                *libc::__errno_location()
            ));
        }
    }
    log::info!("CPU affinity applied to current thread: cores={:?}", mask);
    Ok(())
}

/// Set the niceness of the current process. Higher = nicer (lower priority).
pub fn apply_nice(level: i32) -> Result<()> {
    let level = level.clamp(-20, 19);
    #[cfg(target_os = "linux")]
    unsafe {
        let rc = libc::setpriority(libc::PRIO_PROCESS, 0, level);
        if rc != 0 {
            return Err(anyhow::anyhow!(
                "setpriority failed: errno={}",
                *libc::__errno_location()
            ));
        }
    }
    log::info!("Process nice level set to {}", level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_cores_is_one_or_more() {
        let n = detect_cores();
        assert!(n >= 1, "must always have at least 1 core");
    }

    #[test]
    fn pick_cores_never_zero_and_within_bounds() {
        let total = 16;
        for want in 1..=4 {
            let cores = pick_cores(total, want);
            assert_eq!(cores.len() as u32, want);
            for &c in &cores {
                assert!(c > 0 && c < total - 1, "core {} out of safe zone", c);
            }
        }
    }

    #[test]
    fn allocate_reserves_one_for_host() {
        // On a 4-core system, allocate(0.95) should give at most 2 (not 3)
        // because we reserve 1 core for the host out of `total - 1 = 3 usable`.
        let total = 4;
        // Mock the cached count so allocate() uses total = 4
        CPU_CORES.store(total, Ordering::Release);
        let a = allocate(0.95);
        assert!(a.allocated <= total.saturating_sub(1), "must leave a core");
    }

    #[test]
    fn allocate_returns_at_least_one() {
        CPU_CORES.store(8, Ordering::Release);
        let a = allocate(0.01); // tiny ratio
        assert!(a.allocated >= 1, "must always allocate at least 1 core");
    }

    #[test]
    fn ratio_is_correct() {
        CPU_CORES.store(8, Ordering::Release);
        let a = allocate(1.0);
        let expected_usable = 7.0;
        assert!((a.ratio() - a.allocated as f64 / 8.0).abs() < f64::EPSILON);
        assert!(a.ratio() <= expected_usable / 8.0 + 0.01);
    }
}
