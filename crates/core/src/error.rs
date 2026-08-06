//! Error type for the Polygone core crate.
//!
//! Manual Display impls (no thiserror-style derives on stable Rust 1.79+ to
//! avoid cross-crate thiserror version drift).

#[derive(Debug)]
pub enum PolygoneError {
    /// A fragment with an out-of-range index was supplied.
    InvalidFragmentIndex { index: u8, total: u8 },
    /// Tried to reconstruct with fewer than `threshold` fragments.
    InsufficientFragments { have: usize, need: usize },
    /// Serialization/deserialization failed.
    Serde(String),
    /// Cryptographic operation failed.
    Crypto(String),
    /// I/O failed.
    Io(String),
    /// ML-KEM-1024 decapsulation failed (shared secret mismatch).
    KemDecapsulate,
    /// Shamir split failed.
    ShamirSplit(String),
    /// Shamir reconstruction failed (too few / inconsistent shares).
    ShamirReconstruct(String),
    /// AES-GCM operation failed (tag mismatch / invalid key).
    AeadError(String),
    /// Key parsing / key-file error.
    KeyFile(String),
}

impl std::fmt::Display for PolygoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFragmentIndex { index, total } => {
                write!(f, "fragment index {} out of range [0..{})", index, total)
            }
            Self::InsufficientFragments { have, need } => {
                write!(f, "need {} fragments, only have {}", need, have)
            }
            Self::Serde(m) => write!(f, "serde: {}", m),
            Self::Crypto(m) => write!(f, "crypto: {}", m),
            Self::Io(m) => write!(f, "io: {}", m),
            Self::KemDecapsulate => write!(f, "ML-KEM-1024 decapsulation failed"),
            Self::ShamirSplit(m) => write!(f, "shamir split: {}", m),
            Self::ShamirReconstruct(m) => write!(f, "shamir reconstruct: {}", m),
            Self::AeadError(m) => write!(f, "aead: {}", m),
            Self::KeyFile(m) => write!(f, "key file: {}", m),
        }
    }
}

impl std::error::Error for PolygoneError {}

/// Convenience result alias used across the core crate.
pub type Result<T> = std::result::Result<T, PolygoneError>;

impl From<std::io::Error> for PolygoneError {
    fn from(e: std::io::Error) -> Self { Self::Io(e.to_string()) }
}

impl From<serde_json::Error> for PolygoneError {
    fn from(e: serde_json::Error) -> Self { Self::Serde(e.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let e = PolygoneError::InsufficientFragments { have: 3, need: 4 };
        assert!(e.to_string().contains("need 4"));
        assert!(e.to_string().contains("have 3"));
    }

    #[test]
    fn test_io_conversion() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
        let _: PolygoneError = io.into();
    }
}
