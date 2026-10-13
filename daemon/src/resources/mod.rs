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
        if self.total_bytes == 0 {
            0.0
        } else {
            self.available_bytes as f64 / self.total_bytes as f64 * 100.0
        }
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
        self.interfaces
            .iter()
            .find(|i| !i.is_loopback && i.is_up)
            .map(|i| (i.rx_bytes, i.tx_bytes))
            .unwrap_or((0, 0))
    }

    pub fn total_rx_tx(&self) -> (u64, u64) {
        self.interfaces
            .iter()
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
    /// Capacité de la PLATEFORME (cgroup v2 présent), pas promesse du daemon.
    ///
    /// Depuis 2b-5b, le daemon ne pose plus lui-même de limite mémoire : elle
    /// appartient à `systemd` (`MemoryMax=` de l'unité `--user`). Le drapeau
    /// reste utile — c'est lui qui dira à `install_service` s'il faut émettre un
    /// `MemoryMax=` — mais l'étiquette « Memory limit » du `doctor`
    /// (`main.rs:414`) reste, elle, trompeuse : elle sera corrigée dans
    /// l'incrément qui câblera `install_service`.
    pub memory_limit: bool,
    pub bandwidth_monitor: bool,
    pub gpu_monitor: bool,
    pub named_pipes: bool,
    pub unix_sockets: bool,
    pub cgroups_v2: bool,
    pub launchd: bool,
    pub windows_service: bool,
}

// `CpuAffinityMode` vivait ici (Auto / Spread / Compact / Performance / Off) et
// a été RETIRÉ à l'incrément 2b-8, avec la clé de config `cpu_affinity_mode` qui
// était son unique usage : le mode était lu par personne, et `set_cpu_affinity`
// n'a aucun appelant. Un type public dont la seule raison d'être est une clé
// morte est la même dette que la clé (docs/PLAN_PRIORITES_2B5.md §2).
//
// Le trait `Platform::set_cpu_affinity` reste : il peut servir, et l'enlever
// toucherait linux.rs/macos.rs.

// ============================================================================
// Platform trait
// ============================================================================

/// Ce que l'on sait de l'activité de l'utilisateur devant la machine.
///
/// Trois états, et non un booléen : « je ne sais pas » n'est pas « personne ».
/// Les confondre a fait croire pendant plusieurs incréments qu'un frein
/// fonctionnait alors qu'il ne recevait jamais qu'un `false` de complaisance —
/// voir `docs/PLAN_FREIN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserActivity {
    /// L'utilisateur interagit : le daemon doit s'effacer.
    Active,
    /// Personne devant la machine : le daemon peut prendre la place.
    Idle,
    /// Aucune source de vérité disponible — l'état réel est INCONNU.
    Unknown,
}

impl UserActivity {
    /// L'utilisateur est-il **positivement** devant la machine ?
    ///
    /// Seul `Active` répond oui. `Unknown` répond non, délibérément : on ne
    /// réduit pas l'allocation du réseau sur une ignorance. Confondre « je ne
    /// sais pas » avec « quelqu'un est là » priverait le partage de ressources
    /// sans qu'aucune mesure ne le justifie — l'inverse exact du bug d'origine,
    /// où `false` de complaisance faisait croire au frein qu'il fonctionnait.
    pub fn is_active(self) -> bool {
        matches!(self, UserActivity::Active)
    }
}

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
    //
    // Il n'y a plus de `set_memory_limit` ici, et c'est une décision, pas un
    // oubli : le daemon n'écrit jamais le plafond mémoire du cgroup qui le
    // contient. Ce fichier appartient à `systemd` (`MemoryMax=` de l'unité
    // `--user`) ; un daemon qui le réécrit à chaque tick se bat contre son
    // gestionnaire et plafonne le cgroup de son LANCEUR (terminal, navigateur,
    // session graphique), pas lui-même. Voir `docs/PLAN_PRIORITES_2B5.md` §1/§4.
    fn memory_info(&self) -> anyhow::Result<MemoryInfo>;
    fn process_memory(&self) -> anyhow::Result<ProcessMemory>;

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
    fn user_active(&self) -> anyhow::Result<UserActivity>;
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
compile_error!(
    "polygoned: Windows n'est pas implémenté — le daemon supporte Linux et \
     macOS (cf. ARCHITECTURE.md §11). Définir WindowsPlatform dans \
     resources/windows.rs pour un vrai portage."
);

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!("Polygone daemon only supports Linux, macOS, and Windows");
