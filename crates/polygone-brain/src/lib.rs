//! `polygone-brain` — local quantized LLM inference with Petals fallback.
//!
//! Spec §3: 'Moteur d'intelligence artificielle locale. Intègre un modèle de langage hautement quantifié capable de tourner sur de petites configurations matérielles (ex: PC portable, configurations à ressources contraintes).'
//! Petals = distributed inference fallback across the mesh.

//!Status: stub. Will be implemented in Phase 5. Local model: Notch (1.5B, Qwen2.5 base) at ~/Projets/Notch/.

#![forbid(unsafe_code)]

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder service. Real implementation lives in the corresponding
/// phase of the spec roadmap.
pub struct BrainStub;

impl BrainStub {
    pub fn new() -> Self { Self }
    pub fn label(&self) -> &'static str { "Brain" }
}

impl Default for BrainStub {
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
        let s = BrainStub::new();
        assert_eq!(s.label(), "Brain");
    }
}
