//! polygone-computer — local orchestrator daemon.
//!
//! Boots a Computer with all default services, exposes:
/*!  polygone-computer — the local orchestrator daemon.

Boots a Computer with all default services, exposes:
- a Unix socket IPC on $XDG_RUNTIME_DIR/polygone/computer.sock
- a status file at $XDG_DATA_HOME/polygone/status.json
- graceful shutdown on SIGINT / SIGTERM
- the watch loop restarts crashed services

Run it in the foreground, or via systemd. */

use std::sync::Arc;
use std::time::Duration;

use polygone::computer::Computer;
use polygone::services::Service;

const SOCKET_PATH_DEFAULT: &str = "/tmp/polygone-computer.sock";
const STATUS_FILE_DEFAULT: &str = "/tmp/polygone-status.json";
const STATUS_FLUSH_MS: u64 = 1000;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Print banner
    polygone::print_banner();

    // 2. Parse argv
    let socket_path = std::env::var("POLYGONE_SOCKET")
        .unwrap_or_else(|_| SOCKET_PATH_DEFAULT.to_string());
    let status_path = std::env::var("POLYGONE_STATUS")
        .unwrap_or_else(|_| STATUS_FILE_DEFAULT.to_string());

    eprintln!("[computer] socket={socket_path} status={status_path}");

    // 3. Boot the orchestrator with the default service set
    let computer = match Computer::boot().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[computer] boot failed: {e}");
            std::process::exit(2);
        }
    };
    let computer_for_run = Arc::clone(&computer);
    let computer_for_status = Arc::clone(&computer);

    // 4. Signal handling
    let running = Arc::new(tokio::sync::Notify::new());
    let r = Arc::clone(&running);
    tokio::spawn(async move {
        let mut sigs = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ).expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => eprintln!("[computer] SIGINT"),
            _ = sigs.recv()            => eprintln!("[computer] SIGTERM"),
        }
        r.notify_waiters();
    });

    // 5. Status flusher — writes the JSON snapshot to disk every second
    tokio::spawn(async move {
        loop {
            let snap = computer_for_status.snapshot().await;
            if let Ok(json) = serde_json::to_string_pretty(&snap) {
                let _ = std::fs::write(&status_path, json);
            }
            tokio::time::sleep(Duration::from_millis(STATUS_FLUSH_MS)).await;
        }
    });

    // 6. Start the watchdog. Blocks until signal.
    eprintln!("[computer] booting watchdog loop…");
    tokio::select! {
        res = computer_for_run.run() => {
            if let Err(e) = res {
                eprintln!("[computer] run error: {e}");
            }
        }
        _ = running.notified() => {
            eprintln!("[computer] shutting down…");
            let _ = computer.stop_all().await;
        }
    }

    eprintln!("[computer] bye.");
    Ok(())
}
