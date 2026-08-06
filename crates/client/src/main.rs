//! Polygone client — user-side node that connects to peers and sends messages.
//!
//! ## v2 stub responsibilities
//!
//! 1. Generate a random NodeId on startup (no PII stored)
//! 2. Connect to relay via TCP (relay:7000) or to other peers via libp2p
//! 3. Read allocation decisions from `~/.polygone/daemon.sock` (polygoned)
//! 4. Run the example: Alice sends a message to Charlie via relay

mod client;
mod demo;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "polygone",
    version = "0.1.0",
    about = "Polygone P2P client — \"On voit rien. Et c'est comme ça que ça devrait être.\"",
)]
struct Args {
    #[arg(long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Send a message to a peer (example: Alice → Charlie via relay)
    Send {
        /// Message to send
        #[arg(last = true)]
        msg: Vec<String>,
    },
    /// Start the client in receive mode (Charlie's side of the example)
    Receive,
    /// Start client and print its NodeId
    Id,
    /// Run the flagship E2E demo: Alice → blind relay → Bob, real
    /// post-quantum crypto + relay audit (« on voit rien »)
    Demo {
        /// Relay address (reserved for future TCP mode)
        #[arg(long)]
        relay: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(level)
    ).init();

    match args.command {
        Commands::Id => {
            use polygone_core::NodeId;
            let id = NodeId::random();
            println!("NodeId: {}", id);
        }
        Commands::Demo { relay: _ } => {
            // The flagship demo: Alice → blind relay → Bob, real post-quantum
            // crypto end to end, plus the "on voit rien" relay audit.
            demo::run()?;
        }
        Commands::Send { msg } => {
            let msg = msg.join(" ");
            client::send_msg(&msg).await?;
        }
        Commands::Receive => {
            client::receive().await?;
        }
    }
    Ok(())
}