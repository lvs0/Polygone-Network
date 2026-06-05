//! Error types for the Polygone protocol.

use thiserror::Error;

/// Top-level error type for Polygone operations.
#[derive(Error, Debug)]
pub enum PolygoneError {
    #[error("KEM decapsulation failed")]
    KemDecapsulate,

    #[error("Shamir split error: {0}")]
    ShamirSplit(String),

    #[error("Shamir reconstruction error: {0}")]
    ShamirReconstruct(String),

    #[error("Encryption error: {0}")]
    Encrypt(String),

    #[error("Decryption error: {0}")]
    Decrypt(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Key file error: {0}")]
    KeyFile(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Operation not supported: {0}")]
    Unsupported(String),

    #[error("AEAD error: {0}")]
    AeadError(String),

    #[error("Signature invalid")]
    SignatureInvalid,

    #[error("Topology derivation failed: {0}")]
    TopologyDerivation(String),

    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),

    #[error("Key error: {0}")]
    Key(String),

    #[error("Timeout")]
    Timeout,

    #[error("Peer not found: {0}")]
    PeerNotFound(String),
}

/// Convenience alias for Results using PolygoneError.
pub type PolyResult<T> = Result<T, PolygoneError>;