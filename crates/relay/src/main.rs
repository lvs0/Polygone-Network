//! Polygone relay — "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! This is a **stateless, blind relay**. It receives envelopes from peers,
//! forwards them, and forgets them immediately. It never inspects, logs,
//! or stores the payload of any envelope.
//!
//! ## What the relay actually sees
//!
//! - **Sees**: the routing node ids (`from`, `to`) — authentication and routing
//!   require them — the session id, envelope sizes, and connection timing.
//! - **Never sees**: the payload (KEM ciphertext + Shamir shares are opaque
//!   bytes), the plaintext, or any private key.
//!
//! An earlier version of this header claimed the relay never sees sender or
//! receiver identity. That was false: it routes on node ids and authenticates
//! them (`AUTH_INIT`, `crates/relay/src/relay.rs`). A security claim that the
//! code contradicts is a defect, so it is corrected here rather than kept.
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

use polygone_relay::relay;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Poll-based main — wraps the async relay::run() inside a tiny Tokio runtime.
/// We do this instead of #[tokio::main] because we need conditional setup
/// before the runtime starts.
fn main() -> Result<()> {
    let args = Args::parse();

    if args.command.is_some() && matches!(args.command, Some(Commands::Info)) {
        println!("Polygone Relay v{}", env!("CARGO_PKG_VERSION"));
        println!("AGPL-3.0 — https://github.com/lvs0/Polygone-Network");
        println!("Role: stateless, blind forwarder");
        println!("Transport: TCP (tokio)");
        return Ok(());
    }

    let level = if args.quiet { "warn" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    log::info!(
        "polygone-relay v{} — starting (blind mode)",
        env!("CARGO_PKG_VERSION")
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(relay::run_with_options(
        args.port,
        args.allow_insecure_hello,
    ))
}

/// Make the Commands struct constructible before parsing — for early exit.
#[derive(Parser, Debug)]
#[command(
    name = "polygone-relay",
    version = env!("CARGO_PKG_VERSION"),
    about = "Stateless blind relay for Polygone P2P network"
)]
struct Args {
    #[arg(long, default_value_t = 7000)]
    port: u16,

    #[arg(long, help = "Print only errors and warnings")]
    quiet: bool,

    /// Accept the legacy unauthenticated `HELLO` handshake (insecure).
    #[arg(long, default_value_t = false)]
    allow_insecure_hello: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show relay version and capabilities
    Info,
}
