//! polyGONED — Resource Allocation Daemon for Polygone P2P Network
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! This daemon runs in the background, watches idle resources,
//! and allocates them to the Polygone network — without ever
//! disturbing the operator. Wozniak simplicity. Jobs discipline.

mod allocator;
mod bandwidth;
mod cpu;
mod gpu;
mod socket;
mod system;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static RUNNING: AtomicBool = AtomicBool::new(true);

#[derive(Parser, Debug)]
#[command(
    name = "polygoned",
    version = "0.2.0",
    about = "Lightweight resource daemon for Polygone P2P",
    long_about = None,
)]
struct Args {
    #[arg(long, help = "Don't actually allocate, just print decisions")]
    dry_run: bool,

    #[arg(long, help = "Generate default config file and exit")]
    gen_config: bool,

    #[arg(long, help = "Tick interval in seconds (default: 5)")]
    tick_secs: Option<u64>,

    #[arg(long, help = "CPU allocation ratio 0.0-1.0 (default: auto)")]
    cpu_ratio: Option<f64>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show current allocation and system stats
    Status,
    /// Shrink allocation to zero and exit cleanly
    Stop,
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

    // Handle CLI commands (status / stop)
    if let Some(cmd) = &args.command {
        match cmd {
            Commands::Status => { cmd_status()?; }
            Commands::Stop   => { cmd_stop()?; }
        }
        return Ok(());
    }

    // Generate config and exit if requested
    if args.gen_config {
        gen_config_file()?;
        return Ok(());
    }

    // ── CPU setup — detect cores and apply real affinity ───────────────
    cpu::init();
    let total_cores = cpu::cpu_cores();
    log::info!("polygoned: detected {} logical CPU cores", total_cores);

    // Decide CPU allocation based on RAM-derived ratio or explicit override
    let cpu_target_ratio = args.cpu_ratio.unwrap_or(0.5);
    let cpu_alloc = cpu::allocate(cpu_target_ratio);

    if !args.dry_run {
        // Apply real OS affinity + nice to this process
        if let Err(e) = cpu::apply_thread_affinity(&cpu_alloc.affinity_mask) {
            log::warn!("polygoned: could not set CPU affinity: {}", e);
        } else {
            log::info!("polygoned: CPU affinity set — {} cores allocated, nice={}",
                cpu_alloc.allocated, cpu_alloc.nice);
        }
        if let Err(e) = cpu::apply_nice(cpu_alloc.nice) {
            log::warn!("polygoned: could not set nice level: {}", e);
        }
    } else {
        log::info!("polygoned: dry-run — would set CPU affinity to {} cores (nice={})",
            cpu_alloc.allocated, cpu_alloc.nice);
    }

    log::info!("polygoned v0.2.0 — starting (dry_run={})", args.dry_run);

    socket::ensure_dir()?;

    // Ctrl+C clean shutdown
    {
        ctrlc::set_handler(move || {
            log::info!("polygoned: SIGINT — shrinking and exiting...");
            RUNNING.store(false, Ordering::SeqCst);
        }).ok();
    }

    let tick = Duration::from_secs(args.tick_secs.unwrap_or(5));

    system::refresh();
    let mut allocator = allocator::Allocator::new();
    let mut bw_monitor = bandwidth::Monitor::new(None);
    let mut gpu_alloc = gpu::allocate(None);

    let snap = system::SystemSnapshot::capture();
    let ram_alloc = allocator.tick(&snap);
    let bw = bw_monitor.tick();
    bw_monitor.set_allocated(ram_alloc.bandwidth_mbps);
    gpu_alloc = gpu::allocate(None);

    if !args.dry_run {
        socket::notify_allocation(&ram_alloc, allocator.is_shrinking())?;
        log_alloc(&ram_alloc, &snap, &cpu_alloc, &bw, &gpu_alloc, allocator.is_shrinking());
    } else {
        log_alloc(&ram_alloc, &snap, &cpu_alloc, &bw, &gpu_alloc, allocator.is_shrinking());
    }

    let mut tick_count = 0u64;
    while RUNNING.load(Ordering::SeqCst) {
        tick_count += 1;
        std::thread::sleep(tick);

        let snap = system::SystemSnapshot::capture();
        let ram_alloc = allocator.tick(&snap);
        let shrinking = allocator.is_shrinking();
        let bw = bw_monitor.tick();
        bw_monitor.set_allocated(ram_alloc.bandwidth_mbps);
        gpu_alloc = gpu::allocate(None);

        // Recompute CPU allocation each tick (ratio stays stable but cores adapt)
        let cpu_alloc = cpu::allocate(cpu_target_ratio);

        if !args.dry_run {
            socket::notify_allocation(&ram_alloc, shrinking)?;
        }

        log_alloc(&ram_alloc, &snap, &cpu_alloc, &bw, &gpu_alloc, shrinking);

        if tick_count % 60 == 0 {
            log::info!(
                "polygoned: status | ram={:.1}GB/{:.1}GB cpu={:.0}%({}) alloc={:.1}GB {}",
                snap.used_ram_gb, snap.total_ram_gb, snap.cpu_usage_pct,
                cpu_alloc.allocated, ram_alloc.ram_gb(),
                if shrinking { "SHRINKING" } else { "active" }
            );
        }
    }

    // Clean shutdown
    if !args.dry_run {
        log::info!("polygoned: final shutdown — shrinking RAM allocation to zero");
        allocator.shrink_to_zero();
        let final_alloc = allocator.current();
        let bw = bw_monitor.tick();
        let gpu_alloc = gpu::allocate(None);
        socket::notify_shrink("shutdown")?;
        log_alloc(&final_alloc, &snap, &cpu_alloc, &bw, &gpu_alloc, false);
    }

    log::info!("polygoned: exited cleanly");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────
fn log_alloc(
    alloc: &allocator::Allocation,
    snap: &system::SystemSnapshot,
    cpu: &cpu::CpuAllocation,
    bw: &bandwidth::BandwidthAllocation,
    gpu: &gpu::GpuAllocation,
    shrinking: bool,
) {
    let headroom = snap.free_ram_gb - alloc.ram_gb();
    let state = if shrinking { "SHRINK" } else { "active" };
    log::info!(
        "RAM:{:.1}/{:.1}GB CPU:{:.0}%({}/{} cores) BW:{}Mbps Alloc:{:.1}GB bw_alloc:{}Mbps GPU:{:.0}MiB alloc:{:.0}MiB [{}]",
        snap.used_ram_gb, snap.total_ram_gb,
        snap.cpu_usage_pct,
        cpu.allocated, cpu.total,
        bw.total_mbps(),
        alloc.ram_gb(), bw.alloc_mbps,
        gpu.total_mb, gpu.allocated_mb,
        state,
    );
}

fn cmd_status() -> Result<()> {
    system::refresh();
    let snap = system::SystemSnapshot::capture();
    let alloc = allocator::Allocator::new();

    cpu::init();
    let total_cores = cpu::cpu_cores();
    let cpu_alloc = cpu::allocate(0.5); // status uses a default ratio
    let current = alloc.current();
    let gpu_alloc = gpu::allocate(None);

    println!();
    println!("  ⬡ polyGONED — Status");
    println!("  ──────────────────────────────────────────");
    println!("  System RAM  : {:.1} GB total  |  {:.1} GB free  |  {:.1} GB used",
        snap.total_ram_gb, snap.free_ram_gb, snap.used_ram_gb);
    println!("  CPU cores   : {} total  |  {:.0}% usage  |  {} cores allocated",
        total_cores, snap.cpu_usage_pct, cpu_alloc.allocated);
    println!("  CPU nice    : {}", cpu_alloc.nice);
    println!("  User active : {}", if snap.user_active { "yes" } else { "no" });
    println!();
    println!("  Current RAM allocation  : {:.1} GB", current.ram_gb());
    println!("  Current bandwidth est.  : {} Mbps", current.bandwidth_mbps);
    println!("  GPU memory              : {} MiB total | {} MiB used | {} MiB free",
        gpu_alloc.total_mb, gpu_alloc.used_mb, gpu_alloc.free_mb);
    println!("  GPU allocated to network: {} MiB", gpu_alloc.allocated_mb);
    println!("  Safety margin          : {:.1} GB",
        alloc.config().safety_margin_bytes as f64 / 1_073_741_824.0);
    println!("  Max alloc ratio        : {:.0}%", alloc.config().max_alloc_ratio * 100.0);
    println!("  Ceiling               : {:.1} GB",
        alloc.config().max_alloc_bytes as f64 / 1_073_741_824.0);
    println!();
    println!("  Socket : ~/.polygone/daemon.sock");
    println!("  Version: 0.2.0");
    println!();
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let mut alloc = allocator::Allocator::new();
    alloc.shrink_to_zero();
    let a = alloc.current();
    socket::notify_shrink("user_requested")?;
    println!("polygoned: RAM allocation → {:.1} GB, notified. Exiting.", a.ram_gb());
    Ok(())
}

fn gen_config_file() -> Result<()> {
    let cfg = allocator::Config::default();
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = home.join(".polygone").join("daemon.toml");
    let content = format!(
        "# polyGONED config — {}\\\n\
         # Place at ~/.polygone/daemon.toml\\n\\n\\\n\
         safety_margin_gb = {:.1}\\\n\
         max_alloc_ratio  = {:.2}\\\n\
         min_alloc_mb     = {}\\\n\
         max_alloc_gb     = {:.1}\\\n\
         cpu_ceiling_pct  = {:.0}\\\n",
        chrono_lite(),
        cfg.safety_margin_bytes as f64 / 1_073_741_824.0,
        cfg.max_alloc_ratio,
        cfg.min_alloc_bytes / (1024 * 1024),
        cfg.max_alloc_bytes as f64 / 1_073_741_824.0,
        cfg.cpu_ceiling_pct,
    );
    std::fs::write(&path, &content)?;
    println!("Config written to {}\\n{}", path.display(), content);
    Ok(())
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