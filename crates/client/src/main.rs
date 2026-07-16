//! Polygone client — user-side node that connects to peers and sends messages.
//!
//! ## v2 stub responsibilities
//!
//! 1. Generate a random NodeId on startup (no PII stored)
//! 2. Connect to relay via TCP (relay:7000) or to other peers via libp2p
//! 3. Read allocation decisions from `~/.polygone/daemon.sock` (polygoned)
//! 4. Run the example: Alice sends a message to Charlie via relay

mod client;

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
    /// Run the full E2E demo: Alice → relay → Charlie
    Demo {
        /// Relay address [default: 127.0.0.1:7000]
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
            use polygone_core::{NodeId, Envelope, EnvelopeKind};
            let id = NodeId::random();
            println!("NodeId: {}", id);
        }
        Commands::Demo { relay } => {
            client::demo(relay.unwrap_or_else(|| "127.0.0.1:7000".into())).await?;
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