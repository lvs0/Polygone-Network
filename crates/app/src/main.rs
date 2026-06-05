//! `polygone-app` — the single `polygone` command that dispatches to
//! the TUI master and to all specialized sub-services.
//!
//! Spec §1: "Une seule commande globale dans le terminal."
//! Spec §4: "La commande unique polygone instancie une interface
//! graphique en mode texte (TUI) [...]"
//!
//! Status: stub. The TUI master lives in the legacy `polygone/tui`
//! module today; it will be migrated into this crate in Phase 2.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "polygone", version, about = "Post-quantum ephemeral privacy network")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Launch the TUI master (4 tabs).
    Tui,
    /// Print status of all services.
    Status,
    /// Run all tests.
    Test,
    /// Serve the web admin UI.
    Serve {
        /// Bind address (default 127.0.0.1:8080).
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Start a sub-service daemon.
    Start {
        /// Which sub-service to start.
        service: String,
    },
    /// Stop a sub-service daemon.
    Stop {
        service: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        None | Some(Cmd::Tui) => {
            eprintln!("polygone TUI master — coming in Phase 2");
            eprintln!("(for now use the legacy crate: cargo run -p polygone --bin polygone)");
        }
        Some(Cmd::Status) => {
            eprintln!("msg:    stub");
            eprintln!("drive:  stub");
            eprintln!("hide:   stub");
            eprintln!("mesh:   stub");
            eprintln!("brain:  stub");
        }
        Some(Cmd::Test) => {
            eprintln!("use: cargo test --workspace");
        }
        Some(Cmd::Serve { bind }) => {
            eprintln!("polygone web admin on http://{bind} — coming in Phase 3");
        }
        Some(Cmd::Start { service }) | Some(Cmd::Stop { service }) => {
            eprintln!("{service}: stub (Phase 3+)");
        }
    }
}
