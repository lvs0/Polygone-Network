//! polyGONED — Resource Allocation Daemon for Polygone P2P Network
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! This daemon runs in the background, watches idle resources,
//! and allocates them to the Polygone network — without ever
//! disturbing the operator. Wozniak simplicity. Jobs discipline.
//!
//! Usage:
//!   polyGONed              — start daemon (background)
//!   polyGONed status       — show current allocation
//!   polyGONed stop         — shrink to zero and exit cleanly
//!   polyGONed --dry-run    — print what would happen without acting

mod allocator;
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
    version = "0.1.0",
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
    // Lightweight logger — no external setup needed
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).format(|buf, record| {
        use std::io::Write;
        let t = chrono_lite();
        writeln!(buf, "[polygoned] {} | {}", t, record.args())
    }).init();

    let args = Args::parse();

    // Handle CLI commands (status / stop) — don't daemonize for these
    if let Some(cmd) = &args.command {
        match cmd {
            Commands::Status => cmd_status()?,
            Commands::Stop => cmd_stop()?,
        }
        return Ok(());
    }

    // Generate config and exit if requested
    if args.gen_config {
        gen_config_file()?;
        return Ok(());
    }

    log::info!("polygoned v0.1.0 — starting");
    log::info!("polygoned: dry_run={}", args.dry_run);

    // Ensure ~/.polygone/ exists
    socket::ensure_dir()?;

    // Catch Ctrl+C for clean shutdown — clone the AtomicBool into the handler
    {
        ctrlc::set_handler(move || {
            log::info!("polygoned: received SIGINT, shrinking and exiting...");
            RUNNING.store(false, Ordering::SeqCst);
        }).ok();
    }

    // Tick interval
    let tick = Duration::from_secs(args.tick_secs.unwrap_or(5));

    // Init system once
    system::refresh();

    // Init allocator
    let mut allocator = allocator::Allocator::new();

    // Initial allocation
    let snap = system::SystemSnapshot::capture();
    let alloc = allocator.tick(&snap);

    if !args.dry_run {
        if let Err(e) = socket::notify_allocation(&alloc, allocator.is_shrinking()) {
            log::warn!("polygoned: could not notify initial allocation: {}", e);
        }
        log_alloc(&alloc, &snap, "init");
    } else {
        log_alloc(&alloc, &snap, "init (dry-run)");
    }

    // Main loop
    let mut tick_count = 0u64;
    while RUNNING.load(Ordering::SeqCst) {
        tick_count += 1;
        std::thread::sleep(tick);

        let snap = system::SystemSnapshot::capture();
        let alloc = allocator.tick(&snap);
        let shrinking = allocator.is_shrinking();

        if !args.dry_run {
            if let Err(e) = socket::notify_allocation(&alloc, shrinking) {
                log::debug!("polygoned: socket write: {}", e);
            }
        }

        log_alloc(&alloc, &snap, if shrinking { "shrink" } else { "active" });

        // Every 5 minutes: log a clean status line (for cron/infra checks)
        if tick_count % 60 == 0 {
            let status = format!(
                "ram={:.1}GB/{:.1}GB cpu={:.0}% alloc={:.1}GB {}",
                snap.used_ram_gb, snap.total_ram_gb, snap.cpu_usage_pct,
                alloc.ram_gb(), if shrinking { "SHRINKING" } else { "active" }
            );
            log::info!("polygoned: {}", status);
        }
    }

    // Clean shutdown: shrink to zero
    if !args.dry_run {
        log::info!("polygoned: final shutdown — shrinking to zero");
        allocator.shrink_to_zero();
        let final_alloc = allocator.current();
        if let Err(e) = socket::notify_shrink("shutdown") {
            log::warn!("polygoned: final shrink notify: {}", e);
        }
        log_alloc(&final_alloc, &snap, "shutdown");
    }

    log::info!("polygoned: exited cleanly");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn log_alloc(alloc: &allocator::Allocation, snap: &system::SystemSnapshot, state: &str) {
    let headroom = snap.free_ram_gb - alloc.ram_gb();
    log::info!(
        "RAM:{:.1}/{:.1}GB CPU:{:.0}% Alloc:{:.1}GB headroom:{:.1}GB [{}]",
        snap.used_ram_gb, snap.total_ram_gb, snap.cpu_usage_pct,
        alloc.ram_gb(), headroom, state,
    );
}

fn cmd_status() -> Result<()> {
    system::refresh();
    let snap = system::SystemSnapshot::capture();
    let alloc = allocator::Allocator::new();
    let current = alloc.current();

    println!();
    println!("  polyGONED — Status");
    println!("  ─────────────────────────────");
    println!("  System RAM : {:.1} GB total  |  {:.1} GB free  |  {:.1} GB used",
        snap.total_ram_gb, snap.free_ram_gb, snap.used_ram_gb);
    println!("  CPU cores  : {}  |  {:.0}% usage", snap.cpu_cores, snap.cpu_usage_pct);
    println!("  User active: {}", if snap.user_active { "yes" } else { "no" });
    println!();
    println!("  Current allocation : {:.1} GB RAM  |  {} Mbps bandwidth",
        current.ram_gb(), current.bandwidth_mbps);
    println!("  Config safety margin: {:.1} GB",
        alloc.config().safety_margin_bytes as f64 / 1_073_741_824.0);
    println!("  Config max ratio   : {:.0}%",
        alloc.config().max_alloc_ratio * 100.0);
    println!("  Config ceiling     : {:.1} GB",
        alloc.config().max_alloc_bytes as f64 / 1_073_741_824.0);
    println!();
    println!("  Socket  : ~/.polygone/daemon.sock");
    println!("  Version : 0.1.0");
    println!();
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let mut alloc = allocator::Allocator::new();
    alloc.shrink_to_zero();
    let a = alloc.current();
    socket::notify_shrink("user_requested")?;
    println!("polygoned: allocation shrunk to {:.1} GB, socket notified. Exiting.", a.ram_gb());
    Ok(())
}

fn gen_config_file() -> Result<()> {
    let cfg = allocator::Config::default();
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = home.join(".polygone").join("daemon.toml");
    let content = format!(
        "# polyGONED config — {}\n\
        # Edit and place at ~/.polygone/daemon.toml\n\n\
        safety_margin_gb = {:.1}\n\
        max_alloc_ratio  = {:.2}\n\
        min_alloc_mb     = {}\n\
        max_alloc_gb     = {:.1}\n\
        cpu_ceiling_pct  = {:.0}\n",
        chrono_lite(),
        cfg.safety_margin_bytes as f64 / 1_073_741_824.0,
        cfg.max_alloc_ratio,
        cfg.min_alloc_bytes / (1024 * 1024),
        cfg.max_alloc_bytes as f64 / 1_073_741_824.0,
        cfg.cpu_ceiling_pct,
    );
    std::fs::write(&path, &content)?;
    println!("Config written to {}\n{}", path.display(), content);
    Ok(())
}

fn chrono_lite() -> String {
    let (h, m, s) = {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        let s = (t.as_secs() % 86400) as u32;
        ((s / 3600) % 24, (s / 60) % 60, s % 60)
    };
    format!("{:02}:{:02}:{:02}", h, m, s)
}