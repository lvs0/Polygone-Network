//! `polygone::identity` — ecosystem identity (pseudo + NodeId).
//!
//! Spec §4 (Accueil): "Identité Écosystème : Affichage du
//! pseudonyme de l'utilisateur (configuré lors de l'installation
//! ou généré automatiquement de façon cryptographique)."
//! Spec §5 (installateur): "saisie optionnelle d'un pseudonyme
//! utilisateur (avec génération d'une identité cryptographique
//! aléatoire par défaut si le champ reste vide)."

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where the identity is persisted.
pub fn identity_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".polygone").join("identity.toml")
}

/// User-facing identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    /// User-chosen or auto-generated pseudonym.
    pub pseudo: String,
    /// First 16 hex chars of the SHA-256 of the NodeId, for display.
    pub node_id_short: String,
    /// Unix epoch ms when created.
    pub created_at_ms: u64,
    /// Language code (e.g. "fr", "en"). Spec §5 (installateur).
    pub language: String,
}

impl Identity {
    /// Generate a random 3-syllable pseudo (e.g. "vox-kali-ren").
    pub fn random_pseudo() -> String {
        use rand::seq::SliceRandom;
        const A: &[&str] = &["vox", "khe", "nul", "zar", "phi", "mor", "sha", "xel", "tar", "nym", "lyr", "aes"];
        const B: &[&str] = &["ka", "li", "ri", "on", "an", "ur", "is", "os", "yn", "el"];
        const C: &[&str] = &["ren", "tor", "sec", "men", "the", "dra", "phi", "kos", "rys", "zin"];
        let mut rng = rand::thread_rng();
        format!("{}-{}-{}",
            A.choose(&mut rng).unwrap(),
            B.choose(&mut rng).unwrap(),
            C.choose(&mut rng).unwrap())
    }

    /// Build an identity from a (possibly empty) pseudo.
    pub fn from_pseudo(pseudo: &str, language: &str) -> Self {
        let pseudo = if pseudo.trim().is_empty() {
            Self::random_pseudo()
        } else {
            pseudo.trim().to_string()
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut id = [0u8; 32];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut id);
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(&id);
        let short = hex::encode(&h[..8]);
        Self {
            pseudo,
            node_id_short: short,
            created_at_ms: now,
            language: language.to_string(),
        }
    }
}

/// Load identity from disk, or create a fresh anonymous one.
pub fn load_or_create() -> Identity {
    let p = identity_path();
    if let Ok(s) = std::fs::read_to_string(&p) {
        if let Ok(id) = toml::from_str::<Identity>(&s) {
            return id;
        }
    }
    let id = Identity::from_pseudo("", "fr");
    let _ = save(&id);
    id
}

pub fn save(id: &Identity) -> std::io::Result<()> {
    let p = identity_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(id)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, s)?;
    std::fs::rename(&tmp, &p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_pseudo_has_three_parts() {
        for _ in 0..20 {
            let p = Identity::random_pseudo();
            assert_eq!(p.split('-').count(), 3, "pseudo was {p}");
        }
    }

    #[test]
    fn empty_pseudo_is_replaced_by_random() {
        let id = Identity::from_pseudo("", "fr");
        assert!(!id.pseudo.is_empty());
        assert_eq!(id.pseudo.split('-').count(), 3);
    }

    #[test]
    fn user_pseudo_is_preserved() {
        let id = Identity::from_pseudo("  lévy  ", "en");
        assert_eq!(id.pseudo, "lévy");
        assert_eq!(id.language, "en");
    }

    #[test]
    fn node_id_short_is_16_hex_chars() {
        let id = Identity::from_pseudo("test", "fr");
        assert_eq!(id.node_id_short.len(), 16);
        assert!(id.node_id_short.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let id = Identity::from_pseudo("lévy", "fr");
        save(&id).expect("save");
        // Load via load_or_create() — should match what we saved.
        let loaded = load_or_create();
        assert_eq!(loaded.pseudo, "lévy");
        assert_eq!(loaded.node_id_short, id.node_id_short);
    }
}
