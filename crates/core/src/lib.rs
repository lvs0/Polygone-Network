//! Polygone core — shared types, primitives, wire protocol (no network dependencies).
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."

pub mod crypto;
pub mod envelope;
pub mod error;
pub mod identity;
pub mod sign;

pub use crypto::SharedSecret;
pub use envelope::{Envelope, EnvelopeKind, Fragment, FRAGMENT_SHARES, FRAGMENT_THRESHOLD};
pub use error::{PolygoneError, Result};
pub use identity::{NodeId, SessionId};
pub use sign::{
    KeyPair, PublicKey, SecretKey, Signature, Signer, Verifier, PUBLIC_KEY_SIZE, SECRET_KEY_SIZE,
    SIGNATURE_SIZE,
};

