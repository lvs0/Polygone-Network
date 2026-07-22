//! polygoned v0.3 — Cross-platform resource allocation daemon for Polygone P2P
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//! Lightweight, invisible, gives maximum resources to the network.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use polygoned::{
    create_platform, SystemSnapshot, Allocation,
    GlowUpEngine, DaemonConfig,
    socket::{ensure_dir, notify_allocation, notify_shrink},
};

static RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Parser, Debug)]
#[command(
    name = "polygoned",
    version = "0.3.0",
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
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
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
    }).ok();

    log::info!("polygoned v0.3.0 — starting on {} (dry_run={})", platform.name(), args.dry_run);
    
    ensure_dir()?;

    // Initialize glow-up engine
    let mut engine = GlowUpEngine::new(config, platform);

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

        tick_count += 1;
        if tick_count % 60 == 0 {
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

fn handle_command(cmd: &Commands, config: &DaemonConfig, platform: &dyn polygoned::Platform) -> Result<()> {
    match cmd {
        Commands::Status => {
            let snap = SystemSnapshot::capture(platform);
            let limits = config.effective_limits(&snap);
            let safe_limits = config.apply_safety(limits, &snap);
            
            println!("\n  ⬡ polygoned v0.3.0 — Status");
            println!("  ──────────────────────────────────────────");
            println!("  Platform    : {}", platform.name());
            println!("  Tier        : {}", config.tier);
            println!("  Limits      : CPU {}% | RAM {}% | BW {}% | GPU {}%", 
                safe_limits.max_cpu_percent, safe_limits.max_ram_percent, 
                safe_limits.max_bandwidth_percent, safe_limits.max_gpu_percent);
            println!();
            println!("  System RAM  : {:.1} GB total | {:.1} GB free | {:.1} GB used",
                snap.memory.total_bytes as f64 / 1_073_741_824.0,
                snap.memory.available_bytes as f64 / 1_073_741_824.0,
                snap.memory.used_bytes as f64 / 1_073_741_824.0);
            println!("  CPU         : {} cores | {:.0}% usage | load {:.2}",
                snap.cpu.per_core.len(), snap.cpu.usage_percent,
                snap.cpu.load_average[0]);
            println!("  User active : {}", if snap.user_active { "yes" } else { "no" });
            println!();
            
            for gpu in &snap.gpu {
                println!("  GPU {}       : {} | {:.1} GB VRAM | {} MB used",
                    gpu.device_id, gpu.name,
                    gpu.vram_total_mb as f64 / 1024.0,
                    gpu.vram_used_mb);
            }
            println!();
            println!("  Socket      : ~/.polygone/daemon.sock");
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
    println!("    CPU affinity     : {}", if caps.cpu_affinity { "yes" } else { "no" });
    println!("    CPU priority     : {}", if caps.cpu_priority { "yes" } else { "no" });
    println!("    Memory limit     : {}", if caps.memory_limit { "yes" } else { "no" });
    println!("    Bandwidth monitor: {}", if caps.bandwidth_monitor { "yes" } else { "no" });
    println!("    GPU monitor      : {}", if caps.gpu_monitor { "yes" } else { "no" });
    println!("    Unix sockets     : {}", if caps.unix_sockets { "yes" } else { "no" });
    println!("    Named pipes      : {}", if caps.named_pipes { "yes" } else { "no" });

    if let Ok(cpu) = platform.cpu_info() {
        println!("\n  CPU: {} cores ({} sockets)", cpu.cores, cpu.topology.sockets);
    }

    if let Ok(mem) = platform.memory_info() {
        println!("\n  Memory: {:.1} GB total | {:.1} GB available",
            mem.total_bytes as f64 / 1_073_741_824.0,
            mem.available_bytes as f64 / 1_073_741_824.0);
    }

    if let Ok(bw) = platform.bandwidth_info() {
        println!("\n  Network interfaces:");
        for iface in &bw.interfaces {
            if !iface.name.starts_with("lo") {
                println!("    {} : RX {} MB | TX {} MB", iface.name, iface.rx_bytes / 1_000_000, iface.tx_bytes / 1_000_000);
            }
        }
    }

    if let Ok(gpus) = platform.gpu_info() {
        if !gpus.is_empty() {
            println!("\n  GPUs:");
            for gpu in &gpus {
                println!("    {} [{}]: {} GB | {} MB used | {}°C | {}W", 
                    gpu.device_id, gpu.name, gpu.total_vram_mb as f64 / 1024.0, 
                    gpu.used_vram_mb, gpu.temperature_c, gpu.power_watts);
            }
        } else {
            println!("\n  GPUs: none detected");
        }
    }

    let sock = platform.data_dir().join("ipc").join("polygoned.sock");
    println!("\n  IPC socket: {} {}", sock.display(), if sock.exists() { "✅" } else { "❌" });

    println!("\n  ✅ All checks complete");
}

fn load_or_create_config(args: &Args) -> Result<DaemonConfig> {
    let platform = create_platform();
    let config_path = args.config.as_deref()
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
    
    let content = format!(
        r#"# polygoned config — {}
# Place at {}

[tier]
name = "{}"

[safety]
min_free_ram_gb = {:.1}
min_free_cpu_cores = {}
min_free_vram_mb = {}
max_cpu_percent = {:.0}

[behavior]
tick_interval_secs = {}
grow_step_pct = {}
shrink_step_pct = {}
throttle_on_user_activity = {}
shrink_hysteresis_ticks = {}

[platform]
cpu_affinity_mode = "{}"
memory_limit_enabled = {}
bandwidth_shaping = {}
gpu_allocation_enabled = {}
service_integration = {}
"#,
        chrono_lite(),
        path.display(),
        config.tier,
        config.safety.min_free_ram_gb,
        config.safety.min_free_cpu_cores,
        config.safety.min_free_vram_mb,
        config.safety.max_cpu_percent,
        config.behavior.tick_interval_secs,
        config.behavior.grow_step_pct,
        config.behavior.shrink_step_pct,
        config.behavior.throttle_on_user_activity,
        config.behavior.shrink_hysteresis_ticks,
        config.cpu_affinity_mode,
        config.memory_limit_enabled,
        config.bandwidth_shaping,
        config.gpu_allocation_enabled,
        config.service_integration,
    );
    
    std::fs::write(&path, content)?;
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