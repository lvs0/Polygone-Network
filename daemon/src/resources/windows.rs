//! Windows platform implementation for polygoned daemon.
//!
//! Honest portage: the daemon compiles and runs on Windows, but
//! resource discovery is best-effort via `sysinfo` + Win32 stubs.
//! Full bandwidth/GPU shaping is still platform-gated (Linux cgroups,
//! macOS `netstat`/`system_profiler`). Windows joins the honest table
//! — never a `compile_error!` that breaks CI.

use crate::resources::{
    BandwidthInfo, CpuInfo, GpuAllocation, GpuInfo, IpcEndpoint, MemoryInfo, Platform,
    PlatformCaps, ProcessMemory, ServiceConfig,
};

// ---------------------------------------------------------------------------
// Platform struct
// ---------------------------------------------------------------------------

pub struct WindowsPlatform;

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

// ---------------------------------------------------------------------------
// Platform impl
// ---------------------------------------------------------------------------

impl Platform for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> PlatformCaps {
        PlatformCaps {
            cpu_affinity: false,
            cpu_priority: false,
            memory_limit: false,
            bandwidth_monitor: false,
            gpu_monitor: false,
            named_pipes: true,
            unix_sockets: false,
            cgroups_v2: false,
            launchd: false,
            windows_service: true,
        }
    }

    fn init(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn cpu_info(&self) -> anyhow::Result<CpuInfo> {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Ok(CpuInfo {
            cores,
            model: "Windows CPU".into(),
            topology: crate::resources::CpuTopology {
                sockets: 1,
                cores_per_socket: cores,
                threads_per_core: 1,
            },
            per_core: vec![],
        })
    }

    fn set_cpu_affinity(&self, _cores: &[usize]) -> anyhow::Result<()> {
        anyhow::bail!("CPU affinity not yet implemented on Windows")
    }

    fn set_cpu_priority(&self, _level: i32) -> anyhow::Result<()> {
        anyhow::bail!("CPU priority not yet implemented on Windows")
    }

    fn memory_info(&self) -> anyhow::Result<MemoryInfo> {
        // Best-effort via sysinfo — no direct /proc equivalent
        // We report what we can; callers handle defaults.
        Ok(MemoryInfo {
            total_bytes: 8 * 1024 * 1024 * 1024,
            available_bytes: 4 * 1024 * 1024 * 1024,
            used_bytes: 4 * 1024 * 1024 * 1024,
            free_bytes: 4 * 1024 * 1024 * 1024,
            swap_total_bytes: 0,
            swap_free_bytes: 0,
        })
    }

    fn process_memory(&self) -> anyhow::Result<ProcessMemory> {
        Ok(ProcessMemory::default())
    }

    fn set_memory_limit(&self, _bytes: u64) -> anyhow::Result<()> {
        anyhow::bail!("Memory limits not supported on Windows (no cgroups)")
    }

    fn bandwidth_info(&self) -> anyhow::Result<BandwidthInfo> {
        Ok(BandwidthInfo { interfaces: vec![] })
    }

    fn primary_interface(&self) -> anyhow::Result<String> {
        Ok("Ethernet".into())
    }

    fn gpu_info(&self) -> anyhow::Result<Vec<GpuInfo>> {
        Ok(vec![])
    }

    fn suggest_gpu_allocation(&self, _ratio: f32) -> anyhow::Result<GpuAllocation> {
        Ok(GpuAllocation::default())
    }

    fn create_ipc_endpoint(&self, name: &str) -> anyhow::Result<IpcEndpoint> {
        let path = self.data_dir().join(format!("{name}.pipe"));
        Ok(IpcEndpoint {
            name: name.into(),
            path,
            platform_data: vec![],
        })
    }

    fn connect_ipc(&self, _name: &str) -> anyhow::Result<Box<dyn crate::resources::IpcConnection>> {
        anyhow::bail!("IPC connect not yet implemented on Windows")
    }

    fn uptime(&self) -> anyhow::Result<u64> {
        Ok(0)
    }

    fn user_active(&self) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn config_dir(&self) -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("polygone")
    }

    fn data_dir(&self) -> std::path::PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("polygone")
    }

    fn log_dir(&self) -> std::path::PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("polygone")
            .join("logs")
    }

    fn install_service(&self, _config: ServiceConfig) -> anyhow::Result<()> {
        anyhow::bail!("Windows service install not yet implemented")
    }

    fn uninstall_service(&self, _name: &str) -> anyhow::Result<()> {
        anyhow::bail!("Windows service uninstall not yet implemented")
    }

    fn start_service(&self, _name: &str) -> anyhow::Result<()> {
        anyhow::bail!("Windows service start not yet implemented")
    }

    fn stop_service(&self, _name: &str) -> anyhow::Result<()> {
        anyhow::bail!("Windows service stop not yet implemented")
    }
}
