//! Polygone core — shared types, primitives, wire protocol (no network dependencies).
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."

pub mod envelope;
pub mod error;
pub mod identity;
pub mod time_sync;

pub use envelope::{Envelope, EnvelopeKind, Fragment, FRAGMENT_THRESHOLD, FRAGMENT_SHARES};
pub use error::PolygoneError;
pub use identity::{NodeId, SessionId};
pub use time_sync::{Timestamp, TimeOffset, SyncConfig, SyncStats, PeerTimeState, ClockSource, MedianFilterConfig, WeightedMedianFilter};
