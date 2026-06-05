//! `polygone-drive` — distributed encrypted storage with local web admin UI.
//!
//! Spec §3: 'Système de stockage persistant, distribué et chiffré. Découpe les fichiers, distribue les fragments chiffrés sur les nœuds disponibles et expose une interface d'administration web locale.'


#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder service. Real implementation lives in the corresponding
/// phase of the spec roadmap.
pub struct DriveStub;

impl DriveStub {
    pub fn new() -> Self { Self }
    pub fn label(&self) -> &'static str { "Drive" }
}

impl Default for DriveStub {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_semver() {
        assert!(VERSION.starts_with("1."));
    }
    #[test]
    fn stub_returns_its_label() {
        let s = DriveStub::new();
        assert_eq!(s.label(), "Drive");
    }
}
