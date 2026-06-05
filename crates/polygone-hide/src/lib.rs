//! `polygone-hide` — SOCKS5 anonymizing proxy.
//!
//! Spec §3: 'Couche réseau d'anonymisation bas niveau. Établit un proxy local SOCKS5 pour masquer le routage et interdire l'identification de l'origine physique des paquets.'
//! Default port: 9050 (per spec §4 Paramètres).

//!Status: stub. Will be implemented in Phase 3.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder service. Real implementation lives in the corresponding
/// phase of the spec roadmap.
pub struct HideStub;

impl HideStub {
    pub fn new() -> Self { Self }
    pub fn label(&self) -> &'static str { "Hide" }
}

impl Default for HideStub {
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
        let s = HideStub::new();
        assert_eq!(s.label(), "Hide");
    }
}
