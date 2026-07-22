//! Resource types and Platform abstraction for polygoned daemon.

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform_impl;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform_impl;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform_impl;

pub use platform_impl::*;

// ============================================================================
// Shared data types
// ============================================================================

/// CPU topology (sockets, cores, threads)
#[derive(Debug, Clone, Default)]
pub struct CpuTopology {
    pub sockets: usize,
    pub cores_per_socket: usize,
    pub threads_per_core: usize,
}

/// CPU information
#[derive(Debug, Clone, Default)]
pub struct CpuInfo {
    pub cores: usize,
    pub model: String,
    pub topology: CpuTopology,
    pub per_core: Vec<u32>, // MHz per core
}

/// Memory information
#[derive(Debug, Clone, Default)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_free_bytes: u64,
}

impl MemoryInfo {
    pub fn free_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { self.available_bytes as f64 / self.total_bytes as f64 * 100.0 }
    }
}

/// Network interface statistics
#[derive(Debug, Clone)]
pub struct NetInterface {
    pub name: String,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub speed_mbps: u64,
    pub is_up: bool,
    pub is_loopback: bool,
}

/// Bandwidth information
#[derive(Debug, Clone, Default)]
pub struct BandwidthInfo {
    pub interfaces: Vec<NetInterface>,
}

impl BandwidthInfo {
    pub fn primary_rx_tx(&self) -> (u64, u64) {
        self.interfaces.iter()
            .find(|i| !i.is_loopback && i.is_up)
            .map(|i| (i.rx_bytes, i.tx_bytes))
            .unwrap_or((0, 0))
    }

    pub fn total_rx_tx(&self) -> (u64, u64) {
        self.interfaces.iter()
            .filter(|i| !i.is_loopback)
            .fold((0, 0), |(rx, tx), i| (rx + i.rx_bytes, tx + i.tx_bytes))
    }
}

/// GPU information
#[derive(Debug, Clone, Default)]
pub struct GpuInfo {
    pub device_id: u32,
    pub name: String,
    pub vendor: String,
    pub total_vram_mb: u32,
    pub used_vram_mb: u32,
    pub free_vram_mb: u32,
    pub driver_version: String,
    pub temperature_c: u32,
    pub power_watts: u32,
    pub utilization_pct: u32,
}

impl GpuInfo {
    pub fn free_vram_mb(&self) -> u32 {
        self.total_vram_mb.saturating_sub(self.used_vram_mb)
    }
}

/// GPU allocation suggestion
#[derive(Debug, Clone, Default)]
pub struct GpuAllocation {
    pub device_id: u32,
    pub allocated_mb: u32,
    pub ratio: f32,
}

/// Process memory info
#[derive(Debug, Clone, Default)]
pub struct ProcessMemory {
    pub rss_bytes: u64,
    pub vms_bytes: u64,
    pub working_set_bytes: u64,
}

/// IPC endpoint (server side)
#[derive(Debug, Clone)]
pub struct IpcEndpoint {
    pub name: String,
    pub path: std::path::PathBuf,
    pub platform_data: Vec<u8>,
}

/// IPC connection (client side)
pub trait IpcConnection: Send + Sync {
    fn send(&mut self, data: &[u8]) -> anyhow::Result<()>;
    fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize>;
    fn close(&mut self) -> anyhow::Result<()>;
}

/// Service configuration
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub executable: std::path::PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub auto_start: bool,
    pub user: Option<String>,
}

/// Platform capability flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCaps {
    pub cpu_affinity: bool,
    pub cpu_priority: bool,
    pub memory_limit: bool,
    pub bandwidth_monitor: bool,
    pub gpu_monitor: bool,
    pub named_pipes: bool,
    pub unix_sockets: bool,
    pub cgroups_v2: bool,
    pub launchd: bool,
    pub windows_service: bool,
}

/// CPU affinity mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CpuAffinityMode {
    Auto,
    Spread,
    Compact,
    Performance,
    Off,
}

impl Default for CpuAffinityMode {
    fn default() -> Self { Self::Auto }
}

impl std::fmt::Display for CpuAffinityMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpuAffinityMode::Auto => write!(f, "auto"),
            CpuAffinityMode::Spread => write!(f, "spread"),
            CpuAffinityMode::Compact => write!(f, "compact"),
            CpuAffinityMode::Performance => write!(f, "performance"),
            CpuAffinityMode::Off => write!(f, "off"),
        }
    }
}

impl std::str::FromStr for CpuAffinityMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(CpuAffinityMode::Auto),
            "spread" => Ok(CpuAffinityMode::Spread),
            "compact" => Ok(CpuAffinityMode::Compact),
            "performance" => Ok(CpuAffinityMode::Performance),
            "off" => Ok(CpuAffinityMode::Off),
            _ => Err(format!("unknown cpu affinity mode: {}", s)),
        }
    }
}

// ============================================================================
// Platform trait
// ============================================================================

/// Platform abstraction trait — one implementation per OS.
pub trait Platform: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> PlatformCaps;
    fn init(&mut self) -> anyhow::Result<()>;

    // CPU
    fn cpu_info(&self) -> anyhow::Result<CpuInfo>;
    fn set_cpu_affinity(&self, cores: &[usize]) -> anyhow::Result<()>;
    fn set_cpu_priority(&self, level: i32) -> anyhow::Result<()>;

    // Memory
    fn memory_info(&self) -> anyhow::Result<MemoryInfo>;
    fn process_memory(&self) -> anyhow::Result<ProcessMemory>;
    fn set_memory_limit(&self, bytes: u64) -> anyhow::Result<()>;

    // Bandwidth
    fn bandwidth_info(&self) -> anyhow::Result<BandwidthInfo>;
    fn primary_interface(&self) -> anyhow::Result<String>;

    // GPU
    fn gpu_info(&self) -> anyhow::Result<Vec<GpuInfo>>;
    fn suggest_gpu_allocation(&self, ratio: f32) -> anyhow::Result<GpuAllocation>;

    // IPC
    fn create_ipc_endpoint(&self, name: &str) -> anyhow::Result<IpcEndpoint>;
    fn connect_ipc(&self, name: &str) -> anyhow::Result<Box<dyn IpcConnection>>;

    // System
    fn uptime(&self) -> anyhow::Result<u64>;
    fn user_active(&self) -> anyhow::Result<bool>;
    fn config_dir(&self) -> std::path::PathBuf;
    fn data_dir(&self) -> std::path::PathBuf;
    fn log_dir(&self) -> std::path::PathBuf;

    // Service management
    fn install_service(&self, config: ServiceConfig) -> anyhow::Result<()>;
    fn uninstall_service(&self, name: &str) -> anyhow::Result<()>;
    fn start_service(&self, name: &str) -> anyhow::Result<()>;
    fn stop_service(&self, name: &str) -> anyhow::Result<()>;
}

// ============================================================================
// Factory
// ============================================================================

/// Platform factory — selects the right implementation at runtime.
#[cfg(target_os = "linux")]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(LinuxPlatform::new())
}

#[cfg(target_os = "macos")]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(MacOSPlatform::new())
}

#[cfg(target_os = "windows")]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(WindowsPlatform::new())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("Polygone daemon only supports Linux, macOS, and Windows");