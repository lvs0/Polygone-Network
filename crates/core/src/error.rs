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
        }
    }
}

impl std::error::Error for PolygoneError {}

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
