//! polygoned — Cross-platform resource allocation daemon for Polygone P2P
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//! Lightweight, invisible, gives maximum resources to the network.

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use polygoned::{
    create_platform,
    socket::{notify_allocation, notify_shrink, socket_path},
    Allocation, DaemonConfig, GlowUpEngine, SystemSnapshot,
};

static RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Parser, Debug)]
#[command(
    name = "polygoned",
    version = env!("CARGO_PKG_VERSION"),
    about = "Lightweight resource daemon for Polygone P2P",
    long_about = None,
)]
struct Args {
    #[arg(long, help = "Don't actually allocate, just print decisions")]
    dry_run: bool,

    #[arg(long, help = "Generate default config file and exit")]
    gen_config: bool,

    #[arg(long, help = "Config file path")]
    config: Option<String>,

    #[arg(long, help = "Tier: eco, balanced, performance, max")]
    tier: Option<String>,

    /// Expose a JSON /status endpoint on this address (e.g. 127.0.0.1:9100).
    #[arg(long, value_name = "ADDR")]
    expose: Option<SocketAddr>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show current allocation and system stats
    Status,
    /// Shrink allocation to zero and exit cleanly
    Stop,
    /// Run doctor diagnostics
    Doctor,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(buf, "[polygoned] {} | {}", chrono_lite(), record.args())
        })
        .init();

    let args = Args::parse();

    // Initialize platform
    let mut platform = create_platform();
    platform.init()?;

    // Load or generate config
    let config = load_or_create_config(&args)?;

    // Handle CLI commands
    if let Some(cmd) = &args.command {
        return handle_command(cmd, &config, &*platform);
    }

    if args.gen_config {
        return generate_config_file(&config, &*platform);
    }

    // Setup Ctrl+C handler
    ctrlc::set_handler(move || {
        log::info!("polygoned: SIGINT — shrinking and exiting...");
        RUNNING.store(false, Ordering::SeqCst);
    })
    .ok();

    log::info!(
        "polygoned v{} — starting on {} (dry_run={})",
        env!("CARGO_PKG_VERSION"),
        platform.name(),
        args.dry_run
    );


    // Initialize glow-up engine
    let started_at_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let mut engine = GlowUpEngine::new(config, platform);

    // Shared live-status for --expose HTTP endpoint, if enabled
    let status_state: Arc<std::sync::Mutex<DaemonStatus>> =
        Arc::new(std::sync::Mutex::new(DaemonStatus {
            version: env!("CARGO_PKG_VERSION"),
            tier: engine.config.tier.as_str(),
            started_at_secs: started_at_secs,
            current_alloc: AllocationView::from(&engine.current),
        }));

    // Start HTTP status server if requested
    if let Some(addr) = args.expose {
        let state = Arc::clone(&status_state);
        std::thread::spawn(move || run_status_http(addr, state));
        log::info!("polygoned: HTTP /status exposed on http://{}", addr);
    }

    let tick = Duration::from_secs(engine.config.behavior.tick_interval_secs);
    let mut tick_count = 0u64;

    while RUNNING.load(Ordering::SeqCst) {
        std::thread::sleep(tick);

        // Capture system snapshot
        let snap = SystemSnapshot::capture(&*engine.platform);

        // Compute allocation
        let alloc = match engine.tick(&snap) {
            Ok(a) => a,
            Err(e) => {
                log::error!("tick error: {}", e);
                continue;
            }
        };

        // Apply to platform
        if !args.dry_run {
            if let Err(e) = engine.apply(&alloc) {
                log::warn!("apply failed: {}", e);
            }
            if let Err(e) = notify_allocation(&alloc, alloc.shrinking) {
                log::debug!("notify: {}", e);
            }
        }

        // Log
        log_alloc(&alloc, &snap);

        // Refresh live status for --expose
        if let Ok(mut st) = status_state.lock() {
            st.current_alloc = AllocationView::from(&alloc);
        }

        tick_count += 1;
        if tick_count.is_multiple_of(60) {
            log::info!(
                "polygoned: status | CPU:{:.0}% RAM:{:.1}/{:.1}GB Alloc:{:.1}GB BW:{}Mbps GPU:{}MB tier:{} {}",
                snap.cpu.usage_percent,
                snap.memory.used_bytes as f64 / 1_073_741_824.0,
                snap.memory.total_bytes as f64 / 1_073_741_824.0,
                alloc.ram_bytes as f64 / 1_073_741_824.0,
                alloc.bandwidth_mbps,
                snap.gpu.iter().map(|g| g.vram_total_mb).sum::<u32>(),
                engine.config.tier,
                if alloc.shrinking { "SHRINKING" } else { "active" }
            );
        }
    }

    // Clean shutdown
    if !args.dry_run {
        log::info!("polygoned: final shutdown — shrinking allocation to zero");
        engine.current.ram_bytes = 0;
        engine.current.bandwidth_mbps = 0;
        if let Err(e) = notify_shrink("shutdown") {
            log::debug!("shrink notify: {}", e);
        }
    }

    log::info!("polygoned: exited cleanly");
    Ok(())
}

fn handle_command(
    cmd: &Commands,
    config: &DaemonConfig,
    platform: &dyn polygoned::Platform,
) -> Result<()> {
    match cmd {
        Commands::Status => {
            let snap = SystemSnapshot::capture(platform);
            let limits = config.effective_limits(&snap);
            let safe_limits = config.apply_safety(limits, &snap);

            println!("\n  ⬡ polygoned v{} — Status", env!("CARGO_PKG_VERSION"));
            println!("  ──────────────────────────────────────────");
            println!("  Platform    : {}", platform.name());
            println!("  Tier        : {}", config.tier);
            println!(
                "  Limits      : CPU {}% | RAM {}% | BW {}% | GPU {}%",
                safe_limits.max_cpu_percent,
                safe_limits.max_ram_percent,
                safe_limits.max_bandwidth_percent,
                safe_limits.max_gpu_percent
            );
            println!();
            println!(
                "  System RAM  : {:.1} GB total | {:.1} GB free | {:.1} GB used",
                snap.memory.total_bytes as f64 / 1_073_741_824.0,
                snap.memory.available_bytes as f64 / 1_073_741_824.0,
                snap.memory.used_bytes as f64 / 1_073_741_824.0
            );
            println!(
                "  CPU         : {} cores | {:.0}% usage | load {:.2}",
                snap.cpu.per_core.len(),
                snap.cpu.usage_percent,
                snap.cpu.load_average[0]
            );
            println!(
                "  User active : {}",
                if snap.user_active { "yes" } else { "no" }
            );
            println!();

            for gpu in &snap.gpu {
                println!(
                    "  GPU {}       : {} | {:.1} GB VRAM | {} MB used",
                    gpu.device_id,
                    gpu.name,
                    gpu.vram_total_mb as f64 / 1024.0,
                    gpu.vram_used_mb
                );
            }
            println!();
            let sock = socket_path();
            println!(
                "  Socket      : {} {}",
                sock.display(),
                if sock.exists() { "✅" } else { "❌" }
            );
            println!();
            Ok(())
        }
        Commands::Stop => {
            notify_shrink("user_requested")?;
            println!("polygoned: shrink signal sent.");
            Ok(())
        }
        Commands::Doctor => {
            run_doctor(platform);
            Ok(())
        }
    }
}

fn run_doctor(platform: &dyn polygoned::Platform) {
    println!("\n  ⬡ polygoned doctor");
    println!("  ──────────────────────────────────────────");

    println!("  ✅ Platform: {}", platform.name());
    println!("  Capabilities:");
    let caps = platform.capabilities();
    println!(
        "    CPU affinity     : {}",
        if caps.cpu_affinity { "yes" } else { "no" }
    );
    println!(
        "    CPU priority     : {}",
        if caps.cpu_priority { "yes" } else { "no" }
    );
    println!(
        "    Memory limit     : {}",
        if caps.memory_limit { "yes" } else { "no" }
    );
    println!(
        "    Bandwidth monitor: {}",
        if caps.bandwidth_monitor { "yes" } else { "no" }
    );
    println!(
        "    GPU monitor      : {}",
        if caps.gpu_monitor { "yes" } else { "no" }
    );
    println!(
        "    Unix sockets     : {}",
        if caps.unix_sockets { "yes" } else { "no" }
    );
    println!(
        "    Named pipes      : {}",
        if caps.named_pipes { "yes" } else { "no" }
    );

    if let Ok(cpu) = platform.cpu_info() {
        println!(
            "\n  CPU: {} cores ({} sockets)",
            cpu.cores, cpu.topology.sockets
        );
    }

    if let Ok(mem) = platform.memory_info() {
        println!(
            "\n  Memory: {:.1} GB total | {:.1} GB available",
            mem.total_bytes as f64 / 1_073_741_824.0,
            mem.available_bytes as f64 / 1_073_741_824.0
        );
    }

    if let Ok(bw) = platform.bandwidth_info() {
        println!("\n  Network interfaces:");
        for iface in &bw.interfaces {
            if !iface.name.starts_with("lo") {
                println!(
                    "    {} : RX {} MB | TX {} MB",
                    iface.name,
                    iface.rx_bytes / 1_000_000,
                    iface.tx_bytes / 1_000_000
                );
            }
        }
    }

    if let Ok(gpus) = platform.gpu_info() {
        if !gpus.is_empty() {
            println!("\n  GPUs:");
            for gpu in &gpus {
                println!(
                    "    {} [{}]: {} GB | {} MB used | {}°C | {}W",
                    gpu.device_id,
                    gpu.name,
                    gpu.total_vram_mb as f64 / 1024.0,
                    gpu.used_vram_mb,
                    gpu.temperature_c,
                    gpu.power_watts
                );
            }
        } else {
            println!("\n  GPUs: none detected");
        }
    }

    let sock = socket_path();
    println!(
        "\n  IPC socket: {} {}",
        sock.display(),
        if sock.exists() { "✅" } else { "❌" }
    );

    println!("\n  ✅ All checks complete");
}

fn load_or_create_config(args: &Args) -> Result<DaemonConfig> {
    let platform = create_platform();
    let config_path = args
        .config
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| platform.config_dir().join("daemon.toml"));

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let mut config: DaemonConfig = toml::from_str(&content)?;

        if let Some(tier_str) = &args.tier {
            config.tier = tier_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
            config.custom_limits = None;
        }

        Ok(config)
    } else {
        let mut config = DaemonConfig::default();
        if let Some(tier_str) = &args.tier {
            config.tier = tier_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
            config.custom_limits = None;
        }
        Ok(config)
    }
}

fn generate_config_file(config: &DaemonConfig, platform: &dyn polygoned::Platform) -> Result<()> {
    let path = platform.config_dir().join("daemon.toml");
    std::fs::create_dir_all(platform.config_dir())?;

    // Sérialisé par serde : le format écrit == le format relu par
    // `toml::from_str`. (Bug corrigé : l'ancienne écriture manuelle
    // produisait `[tier] name = …` que le parseur ne pouvait pas relire —
    // `polygoned status` échouait sur la config que le daemon venait
    // d'écrire, et sur toute config d'une version précédente.)
    let content = toml::to_string_pretty(config)?;
    let header = format!(
        "# polygoned config — {}\n# Place at {}\n\n",
        chrono_lite(),
        path.display()
    );

    std::fs::write(&path, format!("{header}{content}"))?;
    println!("Config written to {}", path.display());
    Ok(())
}

fn log_alloc(alloc: &Allocation, snap: &SystemSnapshot) {
    let state = if alloc.shrinking { "SHRINK" } else { "active" };
    log::info!(
        "CPU:{:.0}% RAM:{:.1}/{:.1}GB Alloc:{:.1}GB BW:{}Mbps GPU:{}MB cores:{} [{}]",
        snap.cpu.usage_percent,
        snap.memory.used_bytes as f64 / 1_073_741_824.0,
        snap.memory.total_bytes as f64 / 1_073_741_824.0,
        alloc.ram_bytes as f64 / 1_073_741_824.0,
        alloc.bandwidth_mbps,
        snap.gpu.iter().map(|g| g.vram_total_mb).sum::<u32>(),
        snap.cpu.per_core.len(),
        state
    );
}

fn chrono_lite() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let s = (t.as_secs() % 86400) as u32;
    let h = (s / 3600) % 24;
    let m = (s / 60) % 60;
    let sec = s % 60;
    format!("{:02}:{:02}:{:02}", h, m, sec)
}

/// Serializable view of `Allocation` for the /status endpoint.
#[derive(Debug, Clone, Serialize)]
struct AllocationView {
    ram_bytes: u64,
    ram_gb: f64,
    bandwidth_mbps: u32,
    shrinking: bool,
    shrink_streak: u32,
    free_mem_avg_bytes: u64,
}

impl From<&Allocation> for AllocationView {
    fn from(a: &Allocation) -> Self {
        AllocationView {
            ram_bytes: a.ram_bytes,
            ram_gb: a.ram_bytes as f64 / 1_073_741_824.0,
            bandwidth_mbps: a.bandwidth_mbps,
            shrinking: a.shrinking,
            shrink_streak: a.shrink_streak,
            free_mem_avg_bytes: a.free_mem_avg_bytes,
        }
    }
}

/// Live status snapshot served by `--expose` HTTP endpoint.
#[derive(Debug, Clone, Serialize)]
struct DaemonStatus {
    version: &'static str,
    tier: &'static str,
    started_at_secs: u64,
    current_alloc: AllocationView,
}

/// Minimal HTTP server returning JSON status. No external deps.
fn run_status_http(addr: SocketAddr, state: Arc<std::sync::Mutex<DaemonStatus>>) {
    use std::io::Read;
    use std::net::{TcpListener, TcpStream};

    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            log::error!("polygoned: cannot bind status endpoint {}: {}", addr, e);
            return;
        }
    };
    listener.set_nonblocking(false).ok();
    log::info!("polygoned: status endpoint listening on http://{}", addr);

    for stream in listener.incoming() {
        let mut stream @ TcpStream { .. } = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let n = match stream.read(&mut buf) {
                Ok(n) if n > 0 => n,
                _ => return,
            };
            let req = String::from_utf8_lossy(&buf[..n]);
            let method = req.lines().next().unwrap_or("");
            let path = method.split_whitespace().nth(1).unwrap_or("/");

            let resp = if path == "/status" || path == "/health" {
                let body = {
                    let st = state.lock().unwrap();
                    serde_json::to_string(&*st).unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string())
                };
                format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Access-Control-Allow-Origin: *\r\n\
                     Connection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            } else {
                let body = "{\"error\":\"not_found\"}";
                format!(
                    "HTTP/1.1 404 Not Found\r\n\
                     Content-Type: application/json\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            };
            use std::io::Write;
            let _ = stream.write_all(resp.as_bytes());
        });
    }
}
