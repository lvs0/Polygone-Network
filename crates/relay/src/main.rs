//! Polygone relay — "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! This is a **stateless, blind relay**. It receives envelopes from peers,
//! forwards them, and forgets them immediately. It never inspects, logs,
//! or stores the payload of any envelope.
//!
//! ## Security property (the promise)
//!
//! - Relay sees: `[envelope received from peer A]` → `[forwarded to peer B]`
//! - Relay never sees: sender identity, receiver identity, content, metadata
//!   that proves a conversation happened.
//!
//! ## How it works (v2 — stub)
//!
//! 1. Receive a JSON envelope over TCP ( Tokio)
//! 2. Verify it's a relay-visible kind (Fragment only)
//! 3. Parse just enough to extract the destination peer field
//! 4. Forward the raw JSON blob to the destination
//! 5. Never log the payload content
//!
//! Real implementation: libp2p request-response over relay circuit,
//! encrypted end-to-end, so relay never sees the envelope body.

mod relay;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::future::Future;

/// Poll-based main — wraps the async relay::run() inside a tiny Tokio runtime.
/// We do this instead of #[tokio::main] because we need conditional setup
/// before the runtime starts.
fn main() -> Result<()> {
    let args = Args::parse();

    if args.command.is_some() && matches!(args.command, Some(Commands::Info)) {
        println!("Polygone Relay v0.1.0");
        println!("AGPL-3.0 — https://github.com/lvs0/Polygone-Network");
        println!("Role: stateless, blind forwarder");
        println!("Transport: TCP (tokio)");
        return Ok(());
    }

    let level = if args.quiet { "warn" } else { "info" };
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(level)
    ).init();

    log::info!("polygone-relay v0.1.0 — starting (blind mode)");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(relay::run(args.port))
}

/// Make the Commands struct constructible before parsing — for early exit.
#[derive(Parser, Debug)]
#[command(
    name = "polygone-relay",
    version = "0.1.0",
    about = "Stateless blind relay for Polygone P2P network",
)]
struct Args {
    #[arg(long, default_value_t = 7000)]
    port: u16,

    #[arg(long, help = "Print only errors and warnings")]
    quiet: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show relay version and capabilities
    Info,
}