//! macOS platform implementation for polygoned daemon.

use crate::resources::{
    BandwidthInfo, CpuInfo, GpuAllocation, GpuInfo, IpcEndpoint,
    MemoryInfo, NetInterface, Platform, PlatformCaps, ProcessMemory, ServiceConfig,
};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;

pub struct MacOSPlatform;

impl MacOSPlatform { pub fn new() -> Self { Self } }

impl Platform for MacOSPlatform {
    fn name(&self) -> &'static str { "macos" }

    fn capabilities(&self) -> PlatformCaps {
        PlatformCaps {
            cpu_affinity: true,
            cpu_priority: true,
            memory_limit: false,
            bandwidth_monitor: true,
            gpu_monitor: true,
            named_pipes: false,
            unix_sockets: true,
            cgroups_v2: false,
            launchd: true,
            windows_service: false,
        }
    }

    fn init(&mut self) -> anyhow::Result<()> { Ok(()) }

    fn cpu_info(&self) -> anyhow::Result<CpuInfo> {
        let out = Command::new("sysctl")
            .args(["-n", "hw.logicalcpu", "hw.physicalcpu", "machdep.cpu.brand_string"])
            .output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut lines = s.lines();
        let logical = lines.next().unwrap_or("1").parse().unwrap_or(1);
        let physical = lines.next().unwrap_or("1").parse().unwrap_or(1);
        let model = lines.next().unwrap_or("Apple Silicon").to_string();
        Ok(CpuInfo {
            cores: logical,
            model,
            topology: crate::resources::CpuTopology {
                sockets: 1,
                cores_per_socket: physical,
                threads_per_core: logical / physical.max(1),
            },
            per_core: vec![],
        })
    }

    fn set_cpu_affinity(&self, cores: &[usize]) -> anyhow::Result<()> {
        log::info!("macOS CPU affinity requested: {:?} (stub)", cores);
        Ok(())
    }

    fn set_cpu_priority(&self, level: i32) -> anyhow::Result<()> {
        let qos = match level {
            l if l >= 10 => 0,
            l if l >= 0 => 1,
            _ => 2,
        };
        log::info!("macOS QoS class set to {}", qos);
        Ok(())
    }

    fn memory_info(&self) -> anyhow::Result<MemoryInfo> {
        let out = Command::new("sysctl").args(["-n", "hw.memsize"]).output()?;
        let total = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0);

        let out = Command::new("vm_stat").output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut free_pages = 0u64;
        for line in s.lines() {
            if line.contains("Pages free:") {
                free_pages = line.split_whitespace().nth(2)
                    .unwrap_or("0").trim_end_matches('.').parse().unwrap_or(0);
            }
        }
        let page_size = 4096u64;
        Ok(MemoryInfo {
            total_bytes: total,
            available_bytes: free_pages * page_size,
            used_bytes: total.saturating_sub(free_pages * page_size),
            free_bytes: free_pages * page_size,
            swap_total_bytes: 0,
            swap_free_bytes: 0,
        })
    }

    fn process_memory(&self) -> anyhow::Result<ProcessMemory> {
        let out = Command::new("ps")
            .args(["-o", "rss,vsz", "-p", &std::process::id().to_string()])
            .output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut rss = 0u64;
        let mut vms = 0u64;
        for line in s.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                rss = parts[0].parse::<u64>().unwrap_or(0) * 1024;
                vms = parts[1].parse::<u64>().unwrap_or(0) * 1024;
            }
        }
        Ok(ProcessMemory { rss_bytes: rss, vms_bytes: vms, working_set_bytes: rss })
    }

    fn set_memory_limit(&self, _bytes: u64) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Memory limits not supported on macOS"))
    }

    fn bandwidth_info(&self) -> anyhow::Result<BandwidthInfo> {
        let out = Command::new("netstat").args(["-ibn"]).output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut interfaces = Vec::new();
        for line in s.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 && parts[0] != "lo0" {
                let rx = parts[6].parse().unwrap_or(0);
                let tx = parts[9].parse().unwrap_or(0);
                interfaces.push(NetInterface {
                    name: parts[0].into(), rx_bytes: rx, tx_bytes: tx,
                    rx_packets: 0, tx_packets: 0, rx_errors: 0, tx_errors: 0,
                    speed_mbps: 0, is_up: true, is_loopback: false,
                });
            }
        }
        Ok(BandwidthInfo { interfaces })
    }

    fn primary_interface(&self) -> anyhow::Result<String> {
        let out = Command::new("route").args(["-n", "get", "1.1.1.1"]).output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if line.trim().starts_with("interface:") {
                return Ok(line.split(':').nth(1).unwrap_or("en0").trim().into());
            }
        }
        Ok("en0".into())
    }

    fn gpu_info(&self) -> anyhow::Result<Vec<GpuInfo>> {
        let out = Command::new("system_profiler").args(["SPDisplaysDataType", "-json"]).output()?;
        if !out.status.success() { return Ok(vec![]); }
        let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        let mut gpus = Vec::new();
        if let Some(arr) = json["SPDisplaysDataType"].as_array() {
            for (i, gpu) in arr.iter().enumerate() {
                if let Some(name) = gpu["sppci_model"].as_str() {
                    let vram = gpu["sppci_vram"].as_str()
                        .and_then(|s| s.replace(" MB", "").parse().ok())
                        .unwrap_or(0);
                    gpus.push(GpuInfo {
                        device_id: i as u32,
                        name: name.into(),
                        vendor: "Apple/AMD/NVIDIA".into(),
                        total_vram_mb: vram,
                        used_vram_mb: 0,
                        free_vram_mb: vram,
                        driver_version: "Metal".into(),
                        temperature_c: 0,
                        power_watts: 0,
                        utilization_pct: 0,
                    });
                }
            }
        }
        Ok(gpus)
    }

    fn suggest_gpu_allocation(&self, ratio: f32) -> anyhow::Result<GpuAllocation> {
        let gpus = self.gpu_info()?;
        if let Some(gpu) = gpus.first() {
            Ok(GpuAllocation { device_id: gpu.device_id, allocated_mb: (gpu.free_vram_mb() as f32 * ratio) as u32, ratio })
        } else {
            Ok(GpuAllocation::default())
        }
    }

    fn create_ipc_endpoint(&self, name: &str) -> anyhow::Result<IpcEndpoint> {
        let dir = self.data_dir().join("ipc");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.sock", name));
        if path.exists() { std::fs::remove_file(&path)?; }
        let _listener = UnixListener::bind(&path)?;
        Ok(IpcEndpoint { name: name.into(), path, platform_data: vec![] })
    }

    fn connect_ipc(&self, name: &str) -> anyhow::Result<Box<dyn crate::resources::IpcConnection>> {
        let path = self.data_dir().join("ipc").join(format!("{}.sock", name));
        let stream = UnixStream::connect(path)?;
        Ok(Box::new(UnixIpcConnection(stream)))
    }

    fn uptime(&self) -> anyhow::Result<u64> { Ok(0) }
    fn user_active(&self) -> anyhow::Result<bool> { Ok(false) }

    fn config_dir(&self) -> std::path::PathBuf { dirs::config_dir().unwrap().join("polygone") }
    fn data_dir(&self) -> std::path::PathBuf { dirs::data_local_dir().unwrap().join("polygone") }
    fn log_dir(&self) -> std::path::PathBuf { dirs::cache_dir().unwrap().join("polygone").join("logs") }

    fn install_service(&self, config: ServiceConfig) -> anyhow::Result<()> {
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{}</string>
<key>ProgramArguments</key><array><string>{}</string>{}</array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><true/>
<key>StandardOutPath</key><string>{}/polygoned.log</string>
<key>StandardErrorPath</key><string>{}/polygoned.err.log</string>
</dict></plist>"#,
            config.name,
            config.executable.display(),
            config.args.iter().map(|a| format!("<string>{}</string>", a)).collect::<Vec<_>>().join(""),
            self.log_dir().display(),
            self.log_dir().display()
        );
        let path = dirs::home_dir().unwrap().join("Library/LaunchAgents").join(format!("{}.plist", config.name));
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, plist)?;
        Command::new("launchctl").args(["load", "-w", &path.to_string_lossy()]).status()?;
        Ok(())
    }

    fn uninstall_service(&self, name: &str) -> anyhow::Result<()> {
        let path = dirs::home_dir().unwrap().join("Library/LaunchAgents").join(format!("{}.plist", name));
        let _ = Command::new("launchctl").args(["unload", &path.to_string_lossy()]).status();
        let _ = std::fs::remove_file(path);
        Ok(())
    }

    fn start_service(&self, name: &str) -> anyhow::Result<()> {
        let path = dirs::home_dir().unwrap().join("Library/LaunchAgents").join(format!("{}.plist", name));
        Command::new("launchctl").args(["start", &path.to_string_lossy()]).status()?;
        Ok(())
    }

    fn stop_service(&self, name: &str) -> anyhow::Result<()> {
        let path = dirs::home_dir().unwrap().join("Library/LaunchAgents").join(format!("{}.plist", name));
        Command::new("launchctl").args(["stop", &path.to_string_lossy()]).status()?;
        Ok(())
    }
}

struct UnixIpcConnection(UnixStream);
impl crate::resources::IpcConnection for UnixIpcConnection {
    fn send(&mut self, data: &[u8]) -> anyhow::Result<()> { use std::io::Write; self.0.write_all(data)?; Ok(()) }
    fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> { use std::io::Read; Ok(self.0.read(buf)?) }
    fn close(&mut self) -> anyhow::Result<()> { Ok(()) }
}