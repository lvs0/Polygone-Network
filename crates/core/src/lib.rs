//! Polygone core — shared types, primitives, wire protocol (no network dependencies).
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."

pub mod crypto;
pub mod envelope;
pub mod error;
pub mod identity;
pub mod sign;
pub mod time_sync;

pub use crypto::{SharedSecret};
pub use envelope::{Envelope, EnvelopeKind, Fragment, FRAGMENT_THRESHOLD, FRAGMENT_SHARES};
pub use error::{PolygoneError, Result};
pub use identity::{NodeId, SessionId};
pub use sign::{KeyPair, Signer, Verifier, PublicKey, SecretKey, Signature, SIGNATURE_SIZE, PUBLIC_KEY_SIZE, SECRET_KEY_SIZE};
pub use time_sync::{Timestamp, TimeOffset, SyncConfig, SyncStats, PeerTimeState, ClockSource, MedianFilterConfig, WeightedMedianFilter};
