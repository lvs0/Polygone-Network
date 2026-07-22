//! Linux platform implementation for polygoned daemon.

use crate::resources::{
    BandwidthInfo, CpuInfo, GpuAllocation, GpuInfo, IpcEndpoint,
    MemoryInfo, NetInterface, Platform, PlatformCaps, ProcessMemory, ServiceConfig,
};
use std::fs;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Command;
use libc;

pub struct LinuxPlatform {
    cgroups_v2: bool,
}

impl LinuxPlatform {
    pub fn new() -> Self {
        let cgroups_v2 = std::path::PathBuf::from("/sys/fs/cgroup/cgroup.controllers").exists();
        Self { cgroups_v2 }
    }
}

impl Platform for LinuxPlatform {
    fn name(&self) -> &'static str { "linux" }

    fn capabilities(&self) -> PlatformCaps {
        PlatformCaps {
            cpu_affinity: true,
            cpu_priority: true,
            memory_limit: self.cgroups_v2,
            bandwidth_monitor: true,
            gpu_monitor: true,
            named_pipes: false,
            unix_sockets: true,
            cgroups_v2: self.cgroups_v2,
            launchd: false,
            windows_service: false,
        }
    }

    fn init(&mut self) -> anyhow::Result<()> { Ok(()) }

    fn cpu_info(&self) -> anyhow::Result<CpuInfo> {
        let out = Command::new("lscpu").arg("-J").output()?;
        let json: serde_json::Value = serde_json::from_slice(&out.stdout)?;

        let get = |field: &str| json["lscpu"].as_array()
            .and_then(|a| a.iter().find(|v| v["field"] == field))
            .and_then(|v| v["data"].as_str())
            .unwrap_or("");

        let cores = get("CPU(s)").parse().unwrap_or(1);
        let sockets = get("Socket(s)").parse().unwrap_or(1);
        let cores_per_socket = get("Core(s) per socket").parse().unwrap_or(cores / sockets.max(1));
        let threads_per_core = get("Thread(s) per core").parse().unwrap_or(1);
        let model = get("Model name").to_string();

        let mut per_core = Vec::new();
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("cpu MHz") {
                    if let Some(mhz) = line.split(':').nth(1) {
                        per_core.push(mhz.trim().parse::<f32>().unwrap_or(0.0) as u32);
                    }
                }
            }
        }

        Ok(CpuInfo {
            cores,
            model,
            topology: crate::resources::CpuTopology { sockets, cores_per_socket, threads_per_core },
            per_core,
        })
    }

    fn set_cpu_affinity(&self, cores: &[usize]) -> anyhow::Result<()> {
        let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
        unsafe { libc::CPU_ZERO(&mut set) };
        for &core in cores {
            unsafe { libc::CPU_SET(core, &mut set) };
        }
        let res = unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) };
        if res != 0 {
            anyhow::bail!("sched_setaffinity failed: {}", std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn set_cpu_priority(&self, level: i32) -> anyhow::Result<()> {
        let level = level.clamp(-20, 19);
        let res = unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, level) };
        if res != 0 {
            anyhow::bail!("setpriority failed: {}", std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn memory_info(&self) -> anyhow::Result<MemoryInfo> {
        let content = fs::read_to_string("/proc/meminfo")?;
        let mut info = MemoryInfo::default();
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let val = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                match parts[0] {
                    "MemTotal:" => info.total_bytes = val,
                    "MemFree:" => info.free_bytes = val,
                    "MemAvailable:" => info.available_bytes = val,
                    "SwapTotal:" => info.swap_total_bytes = val,
                    "SwapFree:" => info.swap_free_bytes = val,
                    _ => {}
                }
            }
        }
        info.used_bytes = info.total_bytes.saturating_sub(info.available_bytes);
        Ok(info)
    }

    fn process_memory(&self) -> anyhow::Result<ProcessMemory> {
        let content = fs::read_to_string("/proc/self/status")?;
        let mut mem = ProcessMemory::default();
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                mem.rss_bytes = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            } else if line.starts_with("VmSize:") {
                mem.vms_bytes = line.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0) * 1024;
            }
        }
        Ok(mem)
    }

    fn set_memory_limit(&self, bytes: u64) -> anyhow::Result<()> {
        if !self.cgroups_v2 {
            anyhow::bail!("cgroups v2 not available");
        }
        let cgroup_path = fs::read_to_string("/proc/self/cgroup")?
            .lines()
            .next()
            .and_then(|l| l.split(':').nth(2))
            .map(|p| std::path::PathBuf::from("/sys/fs/cgroup").join(p.trim_start_matches('/')))
            .ok_or_else(|| anyhow::anyhow!("no cgroup"))?;
        fs::write(cgroup_path.join("memory.max"), bytes.to_string())?;
        Ok(())
    }

    fn bandwidth_info(&self) -> anyhow::Result<BandwidthInfo> {
        let content = fs::read_to_string("/proc/net/dev")?;
        let mut interfaces = Vec::new();
        for line in content.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 17 {
                let name = parts[0].trim_end_matches(':');
                if name != "lo" {
                    let rx = parts[1].parse().unwrap_or(0);
                    let tx = parts[9].parse().unwrap_or(0);
                    interfaces.push(NetInterface {
                        name: name.into(),
                        rx_bytes: rx,
                        tx_bytes: tx,
                        rx_packets: parts[2].parse().unwrap_or(0),
                        tx_packets: parts[10].parse().unwrap_or(0),
                        rx_errors: parts[3].parse().unwrap_or(0),
                        tx_errors: parts[11].parse().unwrap_or(0),
                        speed_mbps: 0,
                        is_up: true,
                        is_loopback: false,
                    });
                }
            }
        }
        Ok(BandwidthInfo { interfaces })
    }

    fn primary_interface(&self) -> anyhow::Result<String> {
        let out = Command::new("ip").args(["route", "get", "1.1.1.1"]).output()?;
        let s = String::from_utf8_lossy(&out.stdout);
        // "ip route get 1.1.1.1" output: "... dev wlan0 ..."
        if let Some(dev_idx) = s.find("dev ") {
            let after_dev = &s[dev_idx + 4..];
            let iface = after_dev.split_whitespace().next().unwrap_or("eth0");
            return Ok(iface.into());
        }
        Ok("eth0".into())
    }

    fn gpu_info(&self) -> anyhow::Result<Vec<GpuInfo>> {
        if let Ok(out) = Command::new("nvidia-smi")
            .args(["--query-gpu=index,memory.total,memory.used,name,driver_version,temperature.gpu,power.draw", "--format=csv,noheader,nounits"])
            .output()
        {
            if out.status.success() {
                let mut gpus = Vec::new();
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 7 {
                        gpus.push(GpuInfo {
                            device_id: parts[0].parse().unwrap_or(0),
                            name: parts[3].into(),
                            vendor: "NVIDIA".into(),
                            total_vram_mb: parts[1].parse().unwrap_or(0),
                            used_vram_mb: parts[2].parse().unwrap_or(0),
                            free_vram_mb: 0,
                            driver_version: parts[4].into(),
                            temperature_c: parts[5].parse().unwrap_or(0),
                            power_watts: parts[6].parse::<f32>().unwrap_or(0.0) as u32,
                            utilization_pct: 0,
                        });
                    }
                }
                if !gpus.is_empty() { return Ok(gpus); }
            }
        }
        Ok(vec![])
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
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.sock", name));
        if path.exists() { fs::remove_file(&path)?; }
        let _listener = UnixListener::bind(&path)?;
        Ok(IpcEndpoint { name: name.into(), path, platform_data: vec![] })
    }

    fn connect_ipc(&self, name: &str) -> anyhow::Result<Box<dyn crate::resources::IpcConnection>> {
        let path = self.data_dir().join("ipc").join(format!("{}.sock", name));
        let stream = UnixStream::connect(path)?;
        Ok(Box::new(UnixIpcConnection(stream)))
    }

    fn uptime(&self) -> anyhow::Result<u64> {
        let s = fs::read_to_string("/proc/uptime")?;
        Ok(s.split_whitespace().next().unwrap_or("0").parse::<f64>().unwrap_or(0.0) as u64)
    }

    fn user_active(&self) -> anyhow::Result<bool> { Ok(false) }

    fn config_dir(&self) -> std::path::PathBuf { dirs::config_dir().unwrap().join("polygone") }
    fn data_dir(&self) -> std::path::PathBuf { dirs::data_local_dir().unwrap().join("polygone") }
    fn log_dir(&self) -> std::path::PathBuf { dirs::cache_dir().unwrap().join("polygone").join("logs") }

    fn install_service(&self, config: ServiceConfig) -> anyhow::Result<()> {
        let service = format!(
            r#"[Unit]
Description={}
After=network.target

[Service]
Type=simple
ExecStart={} {}
Environment={}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
            config.description,
            config.executable.display(),
            config.args.join(" "),
            config.env.iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(" ")
        );
        let path = self.config_dir().join("systemd").join("user").join(format!("{}.service", config.name));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, service)?;
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        if config.auto_start {
            let _ = Command::new("systemctl").args(["--user", "enable", "--now", &config.name]).status();
        }
        Ok(())
    }

    fn uninstall_service(&self, name: &str) -> anyhow::Result<()> {
        let _ = Command::new("systemctl").args(["--user", "disable", "--now", name]).status();
        let path = self.config_dir().join("systemd").join("user").join(format!("{}.service", name));
        let _ = fs::remove_file(path);
        let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
        Ok(())
    }

    fn start_service(&self, name: &str) -> anyhow::Result<()> {
        Command::new("systemctl").args(["--user", "start", name]).status()?;
        Ok(())
    }

    fn stop_service(&self, name: &str) -> anyhow::Result<()> {
        Command::new("systemctl").args(["--user", "stop", name]).status()?;
        Ok(())
    }
}

struct UnixIpcConnection(UnixStream);
impl crate::resources::IpcConnection for UnixIpcConnection {
    fn send(&mut self, data: &[u8]) -> anyhow::Result<()> { use std::io::Write; self.0.write_all(data)?; Ok(()) }
    fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> { use std::io::Read; Ok(self.0.read(buf)?) }
    fn close(&mut self) -> anyhow::Result<()> { Ok(()) }
}