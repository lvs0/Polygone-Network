//! `polygone-mesh` — local mesh network (mDNS Wi-Fi + Bluetooth) with load balancing.
//!
//! Spec §3: 'Gestionnaire de transport multi-protocole local. Scanne et interconnecte les machines de l'environnement proche en utilisant le Wi-Fi (via mDNS) et le Bluetooth pour mutualiser les ressources.'
//! Spec §6 (advanced): repartiteur de charge intelligent qui fragmente les tâches lourdes sur la grappe locale de manière chiffrée.

//!Status: stub. Will be implemented in Phase 4.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder service. Real implementation lives in the corresponding
/// phase of the spec roadmap.
pub struct MeshStub;

impl MeshStub {
    pub fn new() -> Self { Self }
    pub fn label(&self) -> &'static str { "Mesh" }
}

impl Default for MeshStub {
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
        let s = MeshStub::new();
        assert_eq!(s.label(), "Mesh");
    }
}
