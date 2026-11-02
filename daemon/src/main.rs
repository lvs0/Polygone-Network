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
    create_platform, socket::socket_path, Allocation, DaemonConfig, GlowUpEngine, SystemSnapshot,
};

use polygoned::runtime::{publish_runtime_state, run_tick, runtime_state};
#[cfg(unix)]
use polygoned::socket::{bind_listener, serve};
use std::sync::RwLock;

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
        return generate_config_file(&config, &*platform, args.config.as_deref());
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

    // État partagé construit depuis une vraie mesure ; run_tick en sera
    // l'unique chemin de publication. DaemonState::new reste réservé aux tests.
    let initial_snapshot = SystemSnapshot::capture(&*engine.platform);
    let shared = Arc::new(RwLock::new(runtime_state(
        &engine,
        &initial_snapshot,
        started_at_secs,
        0,
    )));

    // Le serveur IPC était implémenté et testé, mais jamais démarré par le
    // binaire. En dry-run, aucun socket n'est ouvert.
    #[cfg(unix)]
    let ipc = if args.dry_run {
        None
    } else {
        let path = socket_path();
        let listener = bind_listener(&path)?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_state = Arc::clone(&shared);
        let server_shutdown = Arc::clone(&shutdown);
        let server = std::thread::spawn(move || serve(listener, server_state, server_shutdown));
        log::info!("polygoned: IPC prêt sur {}", path.display());
        Some((shutdown, server, path))
    };

    // Shared live-status for --expose HTTP endpoint, if enabled
    let status_state: Arc<std::sync::Mutex<DaemonStatus>> =
        Arc::new(std::sync::Mutex::new(DaemonStatus {
            version: env!("CARGO_PKG_VERSION"),
            tier: engine.config.tier.as_str(),
            started_at_secs,
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

        // Capture → décision → application éventuelle → publication : un seul
        // chemin testé, y compris en dry-run (sans exposer de socket).
        let seq = tick_count.saturating_add(1);
        let outcome = match run_tick(&mut engine, &shared, started_at_secs, seq, !args.dry_run) {
            Ok(outcome) => outcome,
            Err(error) => {
                log::error!("tick error: {}", error);
                continue;
            }
        };
        let snap = outcome.snapshot;
        let alloc = outcome.allocation;

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

    // Clean shutdown. Publier zéro AVANT de fermer l'IPC, puis arrêter le
    // serveur et retirer uniquement le socket que ce processus vient de créer.
    if !args.dry_run {
        log::info!("polygoned: final shutdown — shrinking allocation to zero");
        engine.current.ram_bytes = 0;
        engine.current.bandwidth_mbps = 0;

        let final_snapshot = SystemSnapshot::capture(&*engine.platform);
        publish_runtime_state(
            &shared,
            runtime_state(&engine, &final_snapshot, started_at_secs, tick_count + 1),
        );

        #[cfg(unix)]
        if let Some((shutdown, server, path)) = ipc {
            shutdown.store(true, Ordering::SeqCst);
            if server.join().is_err() {
                log::error!("polygoned: le thread IPC s'est arrêté sur panic");
            }
            if let Err(error) = std::fs::remove_file(&path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "polygoned: impossible de retirer le socket {}: {error}",
                        path.display()
                    );
                }
            }
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
                match snap.user_active {
                    polygoned::UserActivity::Active => "yes",
                    polygoned::UserActivity::Idle => "no",
                    // Ne jamais afficher « no » pour une ignorance : c'est
                    // exactement le mensonge d'origine (docs/PLAN_FREIN.md §2).
                    polygoned::UserActivity::Unknown => "unknown (no source)",
                }
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
            // Le protocole socket n'expose pas de requête d'arrêt : le daemon
            // s'arrête sur signal système (SIGTERM), que `systemd` envoie via
            // `systemctl --user stop polygoned`. On ne prétend donc pas avoir
            // « envoyé un signal » depuis ici — on indique la vraie commande.
            // L'ancien `notify_shrink` visait une fonction inexistante : il
            // échouait en silence et n'arrêtait rien.
            println!(
                "polygoned: pour arrêter le daemon, utilisez « systemctl --user stop polygoned »."
            );
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

fn generate_config_file(
    config: &DaemonConfig,
    platform: &dyn polygoned::Platform,
    explicit_path: Option<&str>,
) -> Result<()> {
    let path = explicit_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| platform.config_dir().join("daemon.toml"));
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;

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
                    serde_json::to_string(&*st)
                        .unwrap_or_else(|_| "{\"error\":\"serialize\"}".to_string())
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

#[cfg(test)]
mod config_generation_tests {
    use super::*;

    #[test]
    fn explicit_config_path_is_created_and_roundtrips() {
        let unique = format!(
            "polygone-config-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(unique);
        let path = directory.join("custom").join("daemon.toml");
        let platform = create_platform();
        let config = DaemonConfig::default();
        generate_config_file(&config, &*platform, Some(path.to_str().unwrap())).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: DaemonConfig = toml::from_str(&content).unwrap();
        assert_eq!(parsed.tier, config.tier);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
