//! # polygone
//!
//! Post-quantum ephemeral privacy network.
//!
//! ## The idea
//!
//! Classical cryptography hides **content**. It cannot hide that a
//! communication happened. POLYGONE hides the communication itself.
//!
//! A message becomes distributed computational state across 7 ephemeral
//! nodes. Any 4 can reconstruct it. No observer sees more than a fragment.
//! The network dissolves. Keys are zeroed. The exchange did not happen.
//!
//! ```text
//! Alice ──[ML-KEM-1024, one-shot, out-of-band]──► Bob
//!                        │
//!          Ephemeral topology derived from shared secret
//!                        │
//!    ┌───────────────────┴───────────────────┐
//!    │  Shamir 4-of-7 secret fragments       │
//!    │  AES-256-GCM encrypted payload        │
//!    │  BLAKE3 hash commitment              │
//!    └──────────────────────────────────────┘
//! ```
#![allow(missing_docs)]

pub mod compute;
pub mod computer;
pub mod crypto;
pub mod economy;
pub mod identity;
pub mod ipc;
pub mod network;
pub mod protocol;
pub mod server;
pub mod services;
pub mod tui;
pub mod web;

/// Re-export error module so `use crate::error::*` works.
pub mod error {
    pub use super::crypto::error::*;
}

/// Convenience alias for results throughout the crate.
pub type Result<T> = std::result::Result<T, crypto::error::PolygoneError>;

// Re-export core types at crate root.
pub use crypto::error::PolygoneError;
pub use crypto::error::PolyResult;
pub use crypto::SharedSecret;
pub use crypto::KeyPair;
pub use protocol::Session;

// Re-export key P2P types for convenience.
pub use network::{
    P2pNode, P2pConfig, NetworkEvent,
    PolygoneRequest, PolygoneResponse, GossipMessage, Capability,
    NodeId, Topology, TopologyParams,
};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print the polygone ASCII logo banner.
pub fn print_banner() {
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║        ⬡  P O L Y G O N E          ║");
    println!("  ║   Post-quantum ephemeral network    ║");
    println!("  ║     L'information n'existe pas.     ║");
    println!("  ╚══════════════════════════════════════╝");
}