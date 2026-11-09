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
    /// Clés de signature révoquées par une rotation (FV-SEC-05).
    #[serde(default)]
    pub revoked_sign_pk_hex: Vec<String>,
    /// Preuve de la dernière rotation : nouvelle clé signée par l'ancienne.
    #[serde(default)]
    pub last_rotation_proof_hex: Option<String>,
    /// Horodatage de la dernière rotation.
    #[serde(default)]
    pub rotated_at_unix: Option<u64>,
}

/// Résultat d'une rotation de clé de signature.
#[derive(Debug, Clone)]
pub struct RotationOutcome {
    pub revoked_pk_hex: String,
    pub new_pk_hex: String,
    pub rotated_at_unix: u64,
    pub proof_hex: String,
}

/// Message canonique signé lors d'une rotation : la nouvelle clé est liée à
/// l'ancienne, sans réutiliser l'identité KEM (qui reste stable).
pub fn rotation_message(new_pk_hex: &str, rotated_at_unix: u64) -> String {
    format!("polygone-key-rotation-v1\n{new_pk_hex}\n{rotated_at_unix}")
}

fn verify_mode(path: &std::path::Path, expected: u32) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::symlink_metadata(path)?;
        let actual = metadata.permissions().mode() & 0o777;
        if actual != expected {
            anyhow::bail!(
                "permissions insuffisantes pour {} : {:03o}, attendu {:03o}",
                path.display(),
                actual,
                expected
            );
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, expected);
        anyhow::bail!("vérification des permissions privées non implémentée sur cet OS");
    }
    #[cfg(unix)]
    Ok(())
}

fn set_private_permissions(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "permissions privées non implémentées sur cet OS",
        ))
    }
}

pub(crate) fn ensure_private_file(path: &std::path::Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_directory(parent)?;
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        anyhow::bail!(
            "fichier privé non régulier ou lien symbolique refusé : {}",
            path.display()
        );
    }
    set_private_permissions(path)?;
    verify_mode(path, 0o600)
}

pub(crate) fn ensure_private_directory(path: &std::path::Path) -> anyhow::Result<()> {
    if path.file_name() != Some(std::ffi::OsStr::new(".polygone")) {
        anyhow::bail!(
            "refus de durcir un répertoire autre que .polygone : {}",
            path.display()
        );
    }
    std::fs::create_dir_all(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("répertoire privé non sûr : {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
        let mode = std::fs::symlink_metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o700 {
            anyhow::bail!(
                "permissions insuffisantes pour le répertoire privé {} : {:03o}",
                path.display(),
                mode
            );
        }
    }
    Ok(())
}

fn write_private_atomic_with<F>(
    path: &std::path::Path,
    contents: &[u8],
    set_permissions: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&std::path::Path) -> std::io::Result<()>,
{
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("chemin privé sans répertoire parent"))?;
    ensure_private_directory(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("chemin privé sans nom de fichier"))?
        .to_string_lossy();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    let result = (|| -> anyhow::Result<()> {
        file.write_all(contents)?;
        file.sync_all()?;
        set_permissions(&tmp)?;
        verify_mode(&tmp, 0o600)?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        verify_mode(path, 0o600)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub(crate) fn write_private_atomic(path: &std::path::Path, contents: &[u8]) -> anyhow::Result<()> {
    write_private_atomic_with(path, contents, set_private_permissions)
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
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        ensure_private_file(&path)?;
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
            revoked_sign_pk_hex: Vec::new(),
            last_rotation_proof_hex: None,
            rotated_at_unix: None,
        }
    }

    /// FV-SEC-05 — remplace la clé de signature par une neuve, signe la
    /// nouvelle clé avec l'ancienne, puis révoque l'ancienne. L'identité KEM
    /// (donc le `node_id`) ne change pas : les pairs n'ont pas à réapprendre
    /// l'adresse.
    pub fn rotate_sign_key(&mut self) -> anyhow::Result<RotationOutcome> {
        let old_pk_hex = self.sign_pk_hex.clone();
        let old_signer = self.sign_signer()?;
        let fresh = sign::generate_keypair()?;
        let new_pk_hex = fresh.verifier.public_key().to_hex();
        let rotated_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let signature = old_signer.sign(rotation_message(&new_pk_hex, rotated_at_unix).as_bytes());
        let proof_hex = hex::encode(signature.as_bytes());

        self.revoked_sign_pk_hex.push(old_pk_hex.clone());
        self.sign_pk_hex = new_pk_hex.clone();
        self.sign_sk_hex = fresh.signer.secret_key().to_hex();
        self.last_rotation_proof_hex = Some(proof_hex.clone());
        self.rotated_at_unix = Some(rotated_at_unix);

        Ok(RotationOutcome {
            revoked_pk_hex: old_pk_hex,
            new_pk_hex,
            rotated_at_unix,
            proof_hex,
        })
    }

    /// Une clé révoquée ne doit plus être acceptée, même si sa signature est
    /// mathématiquement valide : la révocation est une décision locale.
    pub fn accepts_signature_from(&self, sign_pk_hex: &str) -> bool {
        self.sign_pk_hex == sign_pk_hex
            && !self
                .revoked_sign_pk_hex
                .iter()
                .any(|revoked| revoked == sign_pk_hex)
    }

    /// Persist atomically; never continue if private permissions cannot be enforced.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        let raw = serde_json::to_vec_pretty(self)?;
        write_private_atomic(&path, &raw)
    }

    /// Short display id — first 8 hex chars of the KEM public key.
    pub fn short_id(&self) -> String {
        self.kem_pk_hex.chars().take(8).collect()
    }

    /// Parse the KEM public key back from hex (what you share).
    /// Public API surface; used by downstream crates and tests.
    #[allow(dead_code)]
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
    fn rotation_keeps_identity_and_signs_the_new_key() {
        let mut id = LocalIdentity::generate();
        let kem_before = id.kem_pk_hex.clone();
        let old_pk = id.sign_pk_hex.clone();

        let outcome = id.rotate_sign_key().unwrap();
        assert_eq!(id.kem_pk_hex, kem_before, "le node_id KEM reste stable");
        assert_eq!(outcome.revoked_pk_hex, old_pk);
        assert_ne!(outcome.new_pk_hex, old_pk);

        // La preuve est vérifiable avec l'ancienne clé publique : la nouvelle
        // clé est bien liée à l'ancienne, pas injectée par un tiers.
        let old_verifier = sign::Verifier::from_public(sign::PublicKey::from_hex(&old_pk).unwrap());
        let signature = sign::Signature::from_bytes(&hex::decode(&outcome.proof_hex).unwrap());
        assert!(old_verifier.verify(
            rotation_message(&outcome.new_pk_hex, outcome.rotated_at_unix).as_bytes(),
            &signature
        ));
    }

    #[test]
    fn revoked_key_cannot_sign_anymore() {
        let mut id = LocalIdentity::generate();
        let old_pk = id.sign_pk_hex.clone();
        let outcome = id.rotate_sign_key().unwrap();

        assert!(
            !id.accepts_signature_from(&old_pk),
            "l'ancienne clé doit être révoquée"
        );
        assert!(id.accepts_signature_from(&outcome.new_pk_hex));

        // La nouvelle clé signe réellement et se vérifie avec le verifier courant.
        let message = b"apres rotation";
        let signature = id.sign_signer().unwrap().sign(message);
        assert!(id.sign_verifier().unwrap().verify(message, &signature));
    }

    #[test]
    fn rotation_survives_process_restart() {
        let mut id = LocalIdentity::generate();
        let outcome = id.rotate_sign_key().unwrap();
        let serialized = serde_json::to_string(&id).unwrap();
        let reloaded: LocalIdentity = serde_json::from_str(&serialized).unwrap();

        assert_eq!(reloaded.sign_pk_hex, outcome.new_pk_hex);
        assert!(!reloaded.accepts_signature_from(&outcome.revoked_pk_hex));
        assert_eq!(
            reloaded.revoked_sign_pk_hex,
            vec![outcome.revoked_pk_hex.clone()]
        );
        assert_eq!(
            reloaded.last_rotation_proof_hex.as_deref(),
            Some(outcome.proof_hex.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_write_is_atomic_and_uses_restrictive_modes() {
        let root = std::env::temp_dir().join(format!(
            "polygone-private-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join(".polygone").join("identity.json");
        write_private_atomic(&path, b"private test material").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"private test material");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        ensure_private_file(&path).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn identity_symlink_is_refused_without_changing_its_target() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "polygone-identity-symlink-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = root.join(".polygone");
        std::fs::create_dir_all(&dir).unwrap();
        let target = root.join("outside.json");
        std::fs::write(&target, b"private target").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
        let link = dir.join("identity.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(ensure_private_file(&link).is_err());
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"private target");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_write_refuses_to_change_permissions_of_an_unrelated_directory() {
        let root = std::env::temp_dir().join(format!(
            "polygone-private-scope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("unrelated").join("identity.json");
        assert!(write_private_atomic(&path, b"secret").is_err());
        assert!(!root.exists());
    }

    #[test]
    fn chmod_failure_aborts_private_write_and_removes_temporary_file() {
        let root = std::env::temp_dir().join(format!(
            "polygone-private-fail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join(".polygone").join("identity.json");
        let result = write_private_atomic_with(&path, b"secret", |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "forced chmod failure",
            ))
        });
        let error = result.unwrap_err().to_string();
        assert!(error.contains("forced chmod failure"), "{error}");
        assert!(!path.exists(), "aucun fichier privé ne doit être publié");
        let entries: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(
            entries.is_empty(),
            "fichier temporaire laissé : {entries:?}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn existing_private_file_is_restricted_before_reading() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "polygone-private-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join(".polygone").join("identity.json");
        write_private_atomic(&path, b"secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        ensure_private_file(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pseudo_is_three_syllables() {
        let p = random_pseudo();
        assert_eq!(p.split('-').count(), 3);
    }
}
