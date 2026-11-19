//! Polygone core — shared types, primitives, wire protocol (no network dependencies).
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."

pub mod crypto;
pub mod envelope;
pub mod error;
pub mod evidence;
pub mod identity;
pub mod receipt;
pub mod reuse;
pub mod sign;
pub mod task;

pub use crypto::SharedSecret;
pub use envelope::{Envelope, EnvelopeKind, Fragment, FRAGMENT_SHARES, FRAGMENT_THRESHOLD};
pub use error::{PolygoneError, Result};
pub use evidence::{
    Commitments, Comparison, ComparisonOutcome, ComparisonReason, EvidenceKind, EvidenceMethod,
    EvidenceSubject, EvidenceV1, Provenance, EVIDENCE_SCHEMA_V1,
};
pub use identity::{NodeId, SessionId};
pub use reuse::{suggest_roles, DeviceCapabilities, DeviceRole, RoleSuggestion};
pub use sign::{
    KeyPair, PublicKey, SecretKey, Signature, Signer, Verifier, PUBLIC_KEY_SIZE, SECRET_KEY_SIZE,
    SIGNATURE_SIZE,
};
pub use task::{
    TaskConstraints, TaskError, TaskId, TaskKind, TaskPermissions, TaskRecord, TaskRequirements,
    TaskResult, TaskSpec, TaskState,
};
