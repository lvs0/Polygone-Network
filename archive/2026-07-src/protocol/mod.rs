//! Session protocol: lifecycle, transit, dissolution.

pub mod session;

pub use session::Session;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Instant;

/// Unique session identifier (128 bits, random).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId([u8; 16]);

impl SessionId {
    /// Generate a new random session ID.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
        Self(bytes)
    }
    pub fn to_hex(&self) -> String { hex::encode(&self.0) }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &hex::encode(&self.0[..8]))
    }
}

/// State machine for a POLYGONE session.
///
/// ```text
/// Pending ──establish()──► Established ──send()──► InTransit ──receive()──► Completed
///    │                           │                     │                         │
///    └──────────────── dissolve() ────────────────────────────────────────► Dissolved
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitState {
    /// KEM exchange done, topology not yet derived.
    Pending,
    /// Topology + session key established. Ready to send/receive.
    Established,
    /// Fragments have been dispatched.
    InTransit { dispatched_at: Instant },
    /// All fragments collected and message reconstructed.
    Completed,
    /// Session dissolved — all keying material zeroed.
    Dissolved,
}

impl fmt::Display for TransitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending        => write!(f, "Pending"),
            Self::Established    => write!(f, "Established"),
            Self::InTransit{..}  => write!(f, "InTransit"),
            Self::Completed      => write!(f, "Completed"),
            Self::Dissolved      => write!(f, "Dissolved"),
        }
    }
}

impl TransitState {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Pending       => "⏳",
            Self::Established   => "✓",
            Self::InTransit{..} => "→",
            Self::Completed     => "✔",
            Self::Dissolved     => "○",
        }
    }
}
