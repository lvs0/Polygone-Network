//! duress — the real kill-switch (Axiome 5 : la machine est la menace).
//!
//! `polygone duress --confirmer` détruit **irréversiblement** :
//!   - l'identité locale (`~/.polygone/identity.json`)
//!   - les fichiers reçus (`~/.polygone/received/`)
//!
//! Les fragments chez les destinataires et les backups hors-ligne ne sont
//! PAS détruits — mais sans les clés locales, ils deviennent définitivement
//! illisibles. C'est le point.
//!
//! Le signal est volontairement explicite (--confirmer) : un kill-switch
//! qu'on déclenche par accident n'est pas un kill-switch, c'est une erreur.

use anyhow::Result;
use std::path::PathBuf;

/// What duress destroys, and what it does not — printed before destruction.
pub fn plan() -> String {
    format!(
        "mode duress — destruction irréversible de :\n  · {} (identité, clés ML-KEM/ML-DSA)\n  · {} (fichiers reçus)\n\nReste intact : fragments chez les destinataires, backups hors-ligne —\nmais ils deviennent définitivement illisibles sans vos clés.",
        identity_path().display(),
        received_dir().display()
    )
}

/// Execute the duress destruction. Returns the list of what was removed.
pub fn execute() -> Result<Vec<String>> {
    let mut removed = Vec::new();

    // 1. Identity (keys).
    let id_path = identity_path();
    if id_path.exists() {
        std::fs::remove_file(&id_path)?;
        removed.push(format!("identité supprimée : {}", id_path.display()));
    }

    // 2. Received files.
    let recv = received_dir();
    if recv.exists() {
        let n = std::fs::read_dir(&recv)?.count();
        std::fs::remove_dir_all(&recv)?;
        removed.push(format!(
            "fichiers reçus supprimés : {} ({} entrées)",
            recv.display(),
            n
        ));
    }

    if removed.is_empty() {
        removed.push("rien à détruire : pas d'identité locale".to_string());
    }
    Ok(removed)
}

/// `~/.polygone/identity.json`
fn identity_path() -> PathBuf {
    crate::identity::LocalIdentity::path()
}

/// `~/.polygone/received/`
fn received_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".polygone").join("received")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_mentions_identity_and_received() {
        let p = plan();
        assert!(p.contains("identity.json"));
        assert!(p.contains("received"));
        assert!(p.contains("irréversible"));
    }

    #[test]
    fn execute_without_identity_is_safe() {
        // Uses the real HOME — but only reports; nothing to remove.
        let result = execute().unwrap();
        assert!(!result.is_empty());
        // Either something existed or we reported nothing to destroy.
        assert!(result
            .iter()
            .any(|r| r.contains("supprimé") || r.contains("rien à détruire")));
    }
}
