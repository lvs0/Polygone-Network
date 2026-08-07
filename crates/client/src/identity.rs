//! Local identity — persistent keypair + pseudo for the Polygone product.
//!
//! First run generates an ML-KEM-1024 + ML-DSA-65 keypair and stores it in
//! `~/.polygone/identity.json` (chmod 600). The public key is what you share
//! to receive messages; the secret key never leaves your machine.

use polygone_core::crypto::kem;
use polygone_core::sign;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The persisted local identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalIdentity {
    /// User-chosen or auto-generated pseudonym (e.g. "vox-kali-ren").
    pub pseudo: String,
    /// ML-KEM-1024 public key (hex) — share this to receive messages.
    pub kem_pk_hex: String,
    /// ML-KEM-1024 secret key (hex) — never leaves this machine.
    pub kem_sk_hex: String,
    /// ML-DSA-65 public key (hex) — lets peers authenticate your messages.
    pub sign_pk_hex: String,
    /// ML-DSA-65 secret key (hex) — never leaves this machine.
    pub sign_sk_hex: String,
}

impl LocalIdentity {
    /// Where the identity is persisted.
    pub fn path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".polygone").join("identity.json")
    }

    /// Load the identity from disk, or generate + persist one on first run.
    pub fn load_or_create() -> anyhow::Result<Self> {
        if let Some(id) = Self::load()? {
            return Ok(id);
        }
        let id = Self::generate();
        id.save()?;
        Ok(id)
    }

    /// Load the identity if it exists.
    pub fn load() -> anyhow::Result<Option<Self>> {
        let path = Self::path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)?;
        let id: LocalIdentity = serde_json::from_str(&raw)?;
        Ok(Some(id))
    }

    /// Generate a fresh identity (keys + random pseudo).
    pub fn generate() -> Self {
        let (kem_pk, kem_sk) = kem::generate_keypair().expect("ML-KEM keygen");
        let kp = sign::generate_keypair().expect("ML-DSA keygen");
        Self {
            pseudo: random_pseudo(),
            kem_pk_hex: kem_pk.to_hex(),
            kem_sk_hex: kem_sk.to_hex(),
            sign_pk_hex: kp.verifier.public_key().to_hex(),
            sign_sk_hex: kp.signer.secret_key().to_hex(),
        }
    }

    /// Persist to `~/.polygone/identity.json` with best-effort chmod 600.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Short display id — first 8 hex chars of the KEM public key.
    pub fn short_id(&self) -> String {
        self.kem_pk_hex.chars().take(8).collect()
    }

    /// Parse the KEM public key back from hex (what you share).
    #[allow(dead_code)] // part of the public API surface; exercised in tests
    pub fn kem_public_key(&self) -> anyhow::Result<kem::KemPublicKey> {
        Ok(kem::KemPublicKey::from_hex(&self.kem_pk_hex)?)
    }

    /// Parse the KEM secret key back from hex (only you).
    pub fn kem_secret_key(&self) -> anyhow::Result<kem::KemSecretKey> {
        Ok(kem::KemSecretKey::from_hex(&self.kem_sk_hex)?)
    }

    /// The ML-DSA signer built from the persisted secret key (only you).
    pub fn sign_signer(&self) -> anyhow::Result<sign::Signer> {
        Ok(sign::Signer::from_secret(sign::SecretKey::from_hex(
            &self.sign_sk_hex,
        )?))
    }

    /// The ML-DSA verifier built from the persisted public key (shareable).
    pub fn sign_verifier(&self) -> anyhow::Result<sign::Verifier> {
        Ok(sign::Verifier::from_public(sign::PublicKey::from_hex(
            &self.sign_pk_hex,
        )?))
    }
}

/// Generate a random 3-syllable pseudo (e.g. "vox-kali-ren").
pub fn random_pseudo() -> String {
    use rand::seq::SliceRandom;
    const A: &[&str] = &[
        "vox", "khe", "nul", "zar", "phi", "mor", "sha", "xel", "tar", "nym", "lyr", "aes",
    ];
    const B: &[&str] = &["ka", "li", "ri", "on", "an", "ur", "is", "os", "yn", "el"];
    const C: &[&str] = &[
        "ren", "tor", "sec", "men", "the", "dra", "phi", "kos", "rys", "zin",
    ];
    let mut rng = rand::thread_rng();
    format!(
        "{}-{}-{}",
        A.choose(&mut rng).unwrap(),
        B.choose(&mut rng).unwrap(),
        C.choose(&mut rng).unwrap()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_creates_fresh_keys() {
        let a = LocalIdentity::generate();
        let b = LocalIdentity::generate();
        assert_ne!(a.kem_pk_hex, b.kem_pk_hex);
        assert_ne!(a.sign_pk_hex, b.sign_pk_hex);
        assert_eq!(a.short_id().len(), 8);
    }

    #[test]
    fn generated_keys_roundtrip() {
        let id = LocalIdentity::generate();
        let pk = id.kem_public_key().unwrap();
        let sk = id.kem_secret_key().unwrap();
        // Encapsulate against our own public key and decapsulate — must match.
        let (ct, ss1) = kem::encapsulate(&pk).unwrap();
        let ss2 = kem::decapsulate(&sk, &ct).unwrap();
        assert_eq!(ss1, ss2);
    }

    #[test]
    fn pseudo_is_three_syllables() {
        let p = random_pseudo();
        assert_eq!(p.split('-').count(), 3);
    }
}
