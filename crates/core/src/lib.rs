//! Polygone core — shared types, wire envelope, no network dependencies.
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."

pub mod envelope;
pub mod error;
pub mod identity;

pub use envelope::{Envelope, EnvelopeKind, Fragment, FRAGMENT_THRESHOLD, FRAGMENT_SHARES};
pub use error::PolygoneError;
pub use identity::{NodeId, SessionId};
