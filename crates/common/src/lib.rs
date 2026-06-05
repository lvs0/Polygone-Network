//! `polygone-common` — shared types and serialization.
//!
//! Spec §3: "Structure de données immuable. Gère la sérialisation/
//! désérialisation rapide des paquets réseau (Packet), la gestion
//! des identités cryptographiques des nœuds et les structures
//! d'échange."
//!
//! Status: stub. The existing `polygone/crypto`, `polygone/network`,
//! `polygone/protocol` modules will be migrated here in a follow-up
//! step. For now, the legacy crate at the workspace root exposes
//! them.

#![forbid(unsafe_code)]

pub mod identity;
pub mod packet;

pub use identity::{NodeId, SessionKey};
pub use packet::Packet;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
