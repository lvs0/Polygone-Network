//! Resource sharing and scheduling for the compute daemon.
//!
//! This module manages actual CPU/RAM lending to the network:
//! - Allocates idle resources to remote compute tasks
//! - Schedules task execution with priority queues
//! - Monitors resource consumption per contract
//! - Enforces resource limits to protect the host system
//! - Supports cross-platform resource cgroup/nice enforcement

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::idle::SystemMetrics;

/// Maximum number of concurrent remote tasks we'll accept.
const MAX_CONCURRENT_TASKS: usize = 4;

/// Priority levels for compute tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Background tasks (lowest priority, evictable)
    Background = 0,
    /// Normal compute lending tasks
    Normal = 1,
    /// High-priority tasks (e.g., inference requests)
    High = 2,
    /// Critical tasks (cannot be preempted)
    Critical = 3,
}

/// Resource request from a remote peer.
#[derive(Debug, Clone)]
pub struct ResourceRequest {
    /// Unique request identifier
    pub request_id: String,
    /// Peer ID making the request
    pub peer_id: String,
    /// Type of resource requested
    pub resource_type: ResourceType,
    /// Amount requested (MB for RAM, cores for CPU, GB for storage)
    pub amount: u64,
    /// Priority of the request
    pub priority: TaskPriority,
    /// Maximum duration the peer wants the resource (seconds)
    pub max_duration_secs: u64,
    /// POLY offered for this allocation
    pub poly_offer: f64,
    /// Timestamp when the request was received
    pub received_at: u64,
    /// Whether the request has been accepted
    pub accepted: bool,
}

/// Types of resources that can be shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// RAM in megabytes
    RamMB,
    /// CPU cores/threads
    CpuCores,
    /// Storage in gigabytes
    StorageGB,
    /// GPU compute units
    GpuUnits,
}

impl ResourceType {
    pub fn label(&self) -> &'static str {
        match self {
            ResourceType::RamMB => "RAM (MB)",
            ResourceType::CpuCores => "CPU Cores",
            ResourceType::StorageGB => "Storage (GB)",
            ResourceType::GpuUnits => "GPU Units",
        }
    }
}

/// An active resource allocation to a remote peer.
#[derive(Debug, Clone)]
pub struct ResourceAllocation {
    /// Unique allocation ID
    pub allocation_id: String,
    /// The original request
    pub request: ResourceRequest,
    /// When the allocation started
    pub started_at: u64,
    /// Expected end time (epoch ms)
    pub expires_at: u64,
    /// Actual resource consumed so far
    pub consumed_mb: u64,
    /// CPU time used (seconds)
    pub cpu_seconds_used: f64,
    /// Whether the allocation is currently active
    pub active: bool,
}

impl ResourceAllocation {
    /// How many seconds remain before expiry
    pub fn remaining_secs(&self) -> i64 {
        let now = epoch_ms();
        if now >= self.expires_at {
            0
        } else {
            ((self.expires_at - now) / 1000) as i64
        }
    }

    /// Whether the allocation has expired
    pub fn is_expired(&self) -> bool {
        epoch_ms() >= self.expires_at
    }

    /// POLY earned from this allocation so far
    pub fn poly_earned(&self) -> f64 {
        let elapsed_hrs = (epoch_ms().saturating_sub(self.started_at)) as f64 / 3_600_000.0;
        self.request.poly_offer * elapsed_hrs
    }
}

/// Resource limits for the host system — what we're willing to lend.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Max RAM we'll lend (MB)
    pub max_ram_lendable_mb: u64,
    /// Max CPU cores we'll lend
    pub max_cpu_lendable_cores: u32,
    /// Max fraction of total RAM to lend (0.0–1.0)
    pub ram_lend_fraction: f32,
    /// Max fraction of total CPU to lend (0.0–1.0)
    pub cpu_lend_fraction: f32,
    /// Minimum free RAM to always reserve (MB)
    pub ram_reserve_mb: u64,
    /// Whether lending is globally enabled
    pub lending_enabled: bool,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_ram_lendable_mb: 4096,
            max_cpu_lendable_cores: 4,
            ram_lend_fraction: 0.5,
            cpu_lend_fraction: 0.8,
            ram_reserve_mb: 1024,
            lending_enabled: true,
        }
    }
}

impl ResourceLimits {
    /// Compute actual limits based on current system metrics.
    pub fn effective_limits(&self, metrics: &SystemMetrics) -> EffectiveLimits {
        let total_ram_mb = metrics.ram_total / (1024 * 1024);
        let free_ram_mb = (metrics.ram_total - metrics.ram_used) / (1024 * 1024);
        let lendable_ram = (total_ram_mb as f32 * self.ram_lend_fraction) as u64;
        let available_ram = free_ram_mb.saturating_sub(self.ram_reserve_mb);
        let actual_ram = available_ram.min(lendable_ram).min(self.max_ram_lendable_mb);

        // CPU: estimate available cores from usage
        let cpu_avail_pct = (100.0 - metrics.cpu_usage).max(0.0);
        let estimated_cores = (cpu_avail_pct / 100.0 * 8.0) as u32; // assume 8 cores
        let lendable_cores = (estimated_cores as f32 * self.cpu_lend_fraction) as u32;
        let actual_cores = lendable_cores.min(self.max_cpu_lendable_cores);

        EffectiveLimits {
            lendable_ram_mb: actual_ram,
            lendable_cpu_cores: actual_cores,
            total_ram_mb,
            free_ram_mb,
            cpu_usage_pct: metrics.cpu_usage,
            lending_ok: self.lending_enabled && metrics.is_idle && actual_ram > 256,
        }
    }
}

/// Computed effective lending limits for the current moment.
#[derive(Debug, Clone)]
pub struct EffectiveLimits {
    pub lendable_ram_mb: u64,
    pub lendable_cpu_cores: u32,
    pub total_ram_mb: u64,
    pub free_ram_mb: u64,
    pub cpu_usage_pct: f32,
    pub lending_ok: bool,
}

/// Statistics about resource sharing activity.
#[derive(Debug, Clone, Default)]
pub struct LendingStats {
    /// Total requests received
    pub total_requests: u64,
    /// Total requests accepted
    pub accepted_requests: u64,
    /// Total requests rejected (resource limits)
    pub rejected_requests: u64,
    /// Total RAM lent (MB-hours)
    pub total_ram_lent_mb_hours: f64,
    /// Total CPU lent (core-hours)
    pub total_cpu_lent_core_hours: f64,
    /// Total POLY earned from lending
    pub total_poly_earned: f64,
    /// Total POLY spent on renting
    pub total_poly_spent: f64,
    /// Current number of active allocations
    pub active_allocations: u32,
    /// Peak concurrent allocations
    pub peak_concurrent: u32,
}

/// The resource scheduler — manages lending queue and active allocations.
pub struct ResourceScheduler {
    /// Pending requests waiting for allocation
    pending_queue: VecDeque<ResourceRequest>,
    /// Active allocations
    allocations: Vec<ResourceAllocation>,
    /// Resource limits
    limits: ResourceLimits,
    /// Statistics
    stats: LendingStats,
    /// Global counters (shared with daemon for atomic reads)
    total_earned: Arc<AtomicU64>,
    total_spent: Arc<AtomicU64>,
}

impl ResourceScheduler {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            pending_queue: VecDeque::new(),
            allocations: Vec::new(),
            limits,
            stats: LendingStats::default(),
            total_earned: Arc::new(AtomicU64::new(0)),
            total_spent: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get a reference to the earned counter (for external reads).
    pub fn earned_counter(&self) -> Arc<AtomicU64> {
        self.total_earned.clone()
    }

    /// Get a reference to the spent counter.
    pub fn spent_counter(&self) -> Arc<AtomicU64> {
        self.total_spent.clone()
    }

    /// Enqueue a new resource request from a peer.
    pub fn enqueue_request(&mut self, request: ResourceRequest) {
        self.stats.total_requests += 1;
        // Check if we can even accept more tasks
        if self.allocations.len() >= MAX_CONCURRENT_TASKS {
            self.stats.rejected_requests += 1;
            return;
        }
        self.pending_queue.push_back(request);
    }

    /// Process the pending queue and allocate resources where possible.
    /// Returns newly activated allocations.
    pub fn schedule(&mut self, metrics: &SystemMetrics) -> Vec<ResourceAllocation> {
        let effective = self.limits.effective_limits(metrics);
        let mut new_allocations = Vec::new();

        // First, expire old allocations
        self.expire_allocations();

        // Calculate currently allocated resources
        let mut used_ram = 0u64;
        let mut used_cores = 0u32;
        for alloc in &self.allocations {
            if alloc.active {
                match alloc.request.resource_type {
                    ResourceType::RamMB => used_ram += alloc.request.amount,
                    ResourceType::CpuCores => used_cores += alloc.request.amount as u32,
                    _ => {}
                }
            }
        }

        // Sort pending by priority (highest first)
        let mut sorted: Vec<_> = self.pending_queue.drain(..).collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

        for request in sorted {
            if self.allocations.len() >= MAX_CONCURRENT_TASKS {
                // Put remaining back in queue
                self.pending_queue.push_back(request);
                continue;
            }

            let can_allocate = match request.resource_type {
                ResourceType::RamMB => {
                    used_ram + request.amount <= effective.lendable_ram_mb
                }
                ResourceType::CpuCores => {
                    used_cores + request.amount as u32 <= effective.lendable_cpu_cores
                }
                _ => false, // Storage/GPU not yet enforced
            };

            if can_allocate && effective.lending_ok {
                let now = epoch_ms();
                let alloc = ResourceAllocation {
                    allocation_id: format!("alloc:{:x}", rand::random::<u64>()),
                    request: request.clone(),
                    started_at: now,
                    expires_at: now + request.max_duration_secs * 1000,
                    consumed_mb: 0,
                    cpu_seconds_used: 0.0,
                    active: true,
                };

                match request.resource_type {
                    ResourceType::RamMB => used_ram += request.amount,
                    ResourceType::CpuCores => used_cores += request.amount as u32,
                    _ => {}
                }

                new_allocations.push(alloc.clone());
                self.allocations.push(alloc);
                self.stats.accepted_requests += 1;
                self.stats.active_allocations = self.allocations.iter().filter(|a| a.active).count() as u32;
                if self.stats.active_allocations > self.stats.peak_concurrent {
                    self.stats.peak_concurrent = self.stats.active_allocations;
                }
            } else {
                self.stats.rejected_requests += 1;
                self.pending_queue.push_back(request);
            }
        }

        new_allocations
    }

    /// Expire allocations that have passed their time limit.
    fn expire_allocations(&mut self) {
        for alloc in &mut self.allocations {
            if alloc.active && alloc.is_expired() {
                alloc.active = false;
                // Track lending stats
                let hours = (alloc.expires_at.saturating_sub(alloc.started_at)) as f64 / 3_600_000.0;
                match alloc.request.resource_type {
                    ResourceType::RamMB => {
                        self.stats.total_ram_lent_mb_hours += alloc.request.amount as f64 * hours;
                    }
                    ResourceType::CpuCores => {
                        self.stats.total_cpu_lent_core_hours += alloc.request.amount as f64 * hours;
                    }
                    _ => {}
                }
                let earned = alloc.poly_earned();
                self.stats.total_poly_earned += earned;
                self.total_earned.fetch_add((earned * 1_000_000.0) as u64, Ordering::Relaxed);
            }
        }
        self.stats.active_allocations = self.allocations.iter().filter(|a| a.active).count() as u32;

        // Clean up fully expired allocations (keep last 100 for history)
        if self.allocations.len() > 200 {
            self.allocations.retain(|a| a.active || (epoch_ms() - a.expires_at) < 3_600_000);
        }
    }

    /// Force-cancel all allocations (e.g., user becomes active).
    pub fn cancel_all(&mut self) {
        for alloc in &mut self.allocations {
            alloc.active = false;
        }
        self.pending_queue.clear();
        self.stats.active_allocations = 0;
    }

    /// Get current stats snapshot.
    pub fn stats(&self) -> &LendingStats {
        &self.stats
    }

    /// Get active allocations.
    pub fn active_allocations(&self) -> Vec<&ResourceAllocation> {
        self.allocations.iter().filter(|a| a.active).collect()
    }

    /// Get pending requests count.
    pub fn pending_count(&self) -> usize {
        self.pending_queue.len()
    }

    /// Update resource limits.
    pub fn set_limits(&mut self, limits: ResourceLimits) {
        self.limits = limits;
    }

    /// Record POLY spent (for renting resources from others).
    pub fn record_spend(&self, amount: f64) {
        self.total_spent.fetch_add((amount * 1_000_000.0) as u64, Ordering::Relaxed);
    }
}

/// Simulate resource consumption for an allocation (called periodically).
pub fn simulate_consumption(alloc: &mut ResourceAllocation) {
    if !alloc.active {
        return;
    }
    // Simulate ~50MB/hour RAM consumption, ~0.1 core-hours/hour CPU
    let elapsed_ms = epoch_ms().saturating_sub(alloc.started_at);
    let hours = elapsed_ms as f64 / 3_600_000.0;
    alloc.consumed_mb = (hours * 50.0) as u64;
    alloc.cpu_seconds_used = hours * 3600.0 * 0.1;
}

/// Cross-platform process priority adjustment for lending tasks.
/// On Linux: uses `nice` values. On macOS/Windows: best-effort.
pub fn set_lending_nice() {
    #[cfg(target_os = "linux")]
    {
        // Set nice value to 19 (lowest priority) for lending processes
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        }
    }
    #[cfg(target_os = "macos")]
    {
        // macOS: use setpriority with PRIO_PROCESS
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows: set below-normal process priority via FFI
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn SetPriorityClass(hProcess: *mut std::ffi::c_void, dwPriorityClass: u32) -> i32;
        }
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x00008000;
        unsafe {
            let handle = GetCurrentProcess();
            let _ = SetPriorityClass(handle, BELOW_NORMAL_PRIORITY_CLASS);
        }
    }
}

/// Get the current epoch in milliseconds.
pub fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
