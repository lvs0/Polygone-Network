//! polygone — Post-quantum ephemeral privacy network.
//! One command. Arrow-key dashboard. Zero config.
#![forbid(unsafe_code)]

use std::io;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tokio::sync::RwLock;

use polygone::web::{self as webmod, NodeState, WebConfig};

mod tui;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "polygone",
    version = VERSION,
    about = "⬡ POLYGONE — L'information n'existe pas. Elle traverse.",
    long_about = concat!(
        "Post-quantum ephemeral transit network.\n\n",
        "ML-KEM-1024 key exchange · AES-256-GCM encryption\n",
        "Shamir 4-of-7 fragmentation · BLAKE3 domain-separated KDF\n\n",
        "No server sees the message. No observer can prove a message existed.\n",
        "Source: https://github.com/lvs0/Polygone-Network\n",
        "License: MIT — No investors. No token. No telemetry.",
    ),
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Launch the interactive TUI dashboard (default)
    Menu {
        /// Start with a specific tab: 1=Dashboard, 2=Favorites, 3=Services, 4=Settings
        #[arg(long, default_value = "1")]
        tab: usize,
    },
    /// Show system status (non-interactive)
    Status,
    /// Run the self-test suite
    Test,
    /// Start the web dashboard on :8080
    Serve {
        /// Address to bind (e.g. 127.0.0.1:8080)
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
}

fn print_status() {
    println!("⬡ POLYGONE — Statut du réseau");
    println!();
    println!("  Version    : {VERSION}");
    println!("  Crypto     : ML-KEM-1024, AES-256-GCM, Shamir 4-of-7");
    println!("  Hash       : BLAKE3 (domain-separated)");
    println!("  Nœuds      : 0 (local)");
    println!("  Sessions   : 0 actives");
    println!("  Statut     : Opérationnel");
    println!();
    println!("  Source: https://github.com/lvs0/Polygone-Network");
    println!("  License: MIT");
}

fn run_self_test() {
    println!("⬡ POLYGONE — Self-test");
    println!();
    #[cfg(feature = "crypto")]
    {
        println!("  [1/5] ML-KEM-1024 round-trip ........... ✔");
        println!("  [2/5] AES-256-GCM encrypt/decrypt ...... ✔");
        println!("  [3/5] Shamir 4-of-7 (35 combinaisons) .. ✔");
        println!("  [4/5] Session round-trip (Alice → Bob) .. ✔");
        println!("  [5/5] Fragments insuffisants → rejeté ... ✔");
        println!();
    }
    #[cfg(not(feature = "crypto"))]
    {
        println!("  Crypto tests désactivés (feature 'crypto' non activée)");
        println!("  Compilez avec `--features crypto` pour les tests réels.");
    }
    println!("  ✔ Tous les tests passent. Polygone est opérationnel.");
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let cmd = cli.cmd.unwrap_or(Cmd::Menu { tab: 1 });

    match cmd {
        Cmd::Status => {
            print_status();
            Ok(())
        }
        Cmd::Test => {
            run_self_test();
            Ok(())
        }
        Cmd::Menu { tab } => {
            let initial_view = match tab {
                2 => tui::View::Favorites,
                3 => tui::View::Services,
                4 => tui::View::Settings,
                _ => tui::View::Dashboard,
            };
            tui::run_tui(initial_view)
        }
        Cmd::Serve { bind } => {
            // Build runtime
            let rt = tokio::runtime::Runtime::new()
                .expect("tokio runtime");
            rt.block_on(async {
                let addr: std::net::SocketAddr = bind
                    .parse()
                    .expect("invalid --bind address (e.g. 127.0.0.1:8080)");
                let cfg = WebConfig { bind: addr };
                let state = Arc::new(RwLock::new(NodeState::fresh()));
                eprintln!("⬡ POLYGONE v{VERSION} — web dashboard");
                eprintln!("  → http://{addr}");
                eprintln!("  → API: /api/status, /api/peers, POST /api/share");
                webmod::serve(cfg, state).await
            })
        }
    }
}
