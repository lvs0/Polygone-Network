//! duress — the real kill-switch (Axiome 5 : la machine est la menace).
//!
//! `polygone duress --confirmer` détruit **irréversiblement** tout l'état
//! local de Polygone :
//!   - l'identité locale (`~/.polygone/identity.json`)
//!   - les fichiers reçus (`~/.polygone/received/`)
//!   - l'état RES local (`~/.polygone/reputation.json`)
//!   - les ancres de confiance (`~/.polygone/peers.json`) — la trace de
//!     *qui* on a contacté (TOFU : node_id → empreinte ML-DSA)
//!
//! Les fragments chez les destinataires et les backups hors-ligne ne sont
//! PAS détruits — mais sans les clés locales, ils deviennent définitivement
//! illisibles. C'est le point.
//!
//! Limite assumée : les journaux shell (ex. `~/.bash_history`) ne sont pas
//! touchés — c'est l'état de l'utilisateur, pas de l'application. Pour les
//! messages sensibles, préférez la TUI (`polygone`) ou le pipe stdin au
//! passage d'arguments en clair.
//!
//! Le signal est volontairement explicite (--confirmer) : un kill-switch
//! qu'on déclenche par accident n'est pas un kill-switch, c'est une erreur.

use anyhow::Result;
use std::path::PathBuf;

/// What duress destroys, and what it does not — printed before destruction.
pub fn plan() -> String {
    format!(
        "mode duress — destruction irréversible de :\n  · {} (identité, clés ML-KEM/ML-DSA)\n  · {} (fichiers reçus)\n  · {} (état RES local)\n  · {} (ancres de confiance — qui vous avez contacté)\n\nReste intact : fragments chez les destinataires, backups hors-ligne —\nmais ils deviennent définitivement illisibles sans vos clés.\nJournaux shell : hors du périmètre (état de l'utilisateur, pas de l'app).",
        identity_path().display(),
        received_dir().display(),
        reputation_path().display(),
        peers_path().display()
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

    // 3. Local RES state (reputation of peers is a trace of past sessions).
    let rep = reputation_path();
    if rep.exists() {
        std::fs::remove_file(&rep)?;
        removed.push(format!("état RES supprimé : {}", rep.display()));
    }

    // 4. Trust anchors (peers.json) — the relational trace: who was contacted.
    let peers = peers_path();
    if peers.exists() {
        std::fs::remove_file(&peers)?;
        removed.push(format!(
            "ancres de confiance supprimées : {}",
            peers.display()
        ));
    }

    if removed.is_empty() {
        removed.push("rien à détruire : pas d'état local".to_string());
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

/// `~/.polygone/reputation.json`
fn reputation_path() -> PathBuf {
    crate::reputation::ReputationTable::path()
}

/// `~/.polygone/peers.json` (TOFU anchors)
fn peers_path() -> PathBuf {
    crate::net::peers_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HOME est un état global du process : prendre le verrou partagé
    /// (testutil) pour ne jamais marcher sur les tests qui en dépendent
    /// (ex. le sandbox systemd d'exec). Un test de duress ne doit JAMAIS
    /// toucher au vrai HOME.
    fn with_isolated_home(f: impl FnOnce()) {
        let _g = crate::testutil::with_global_state_guard();
        let prev = std::env::var_os("HOME");
        let tmp = std::env::temp_dir().join(format!(
            "duress-{}-{:?}",
            std::process::id(),
            std::thread::current().name()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("HOME", &tmp);
        f();
        match prev {
            Some(p) => std::env::set_var("HOME", p),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn plan_mentions_all_targets() {
        let p = plan();
        assert!(p.contains("identity.json"));
        assert!(p.contains("received"));
        assert!(p.contains("reputation.json"));
        assert!(p.contains("peers.json"));
        assert!(p.contains("irréversible"));
    }

    #[test]
    fn execute_on_empty_home_is_safe() {
        with_isolated_home(|| {
            let result = execute().unwrap();
            assert!(
                result.iter().any(|r| r.contains("rien à détruire")),
                "sur HOME vide, duress doit le dire : {result:?}"
            );
        });
    }

    #[test]
    fn execute_destroys_every_local_state() {
        with_isolated_home(|| {
            // Reproduire un état local complet.
            let pg = std::env::var_os("HOME")
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let base = PathBuf::from(&pg).join(".polygone");
            std::fs::create_dir_all(base.join("received")).unwrap();
            std::fs::write(base.join("identity.json"), b"keys").unwrap();
            std::fs::write(base.join("reputation.json"), b"{}").unwrap();
            std::fs::write(base.join("peers.json"), b"{}").unwrap();
            std::fs::write(base.join("received").join("f"), b"data").unwrap();

            let removed = execute().unwrap();
            assert_eq!(removed.len(), 4, "4 cibles détruites : {removed:?}");

            assert!(!base.join("identity.json").exists());
            assert!(!base.join("reputation.json").exists());
            assert!(!base.join("peers.json").exists());
            assert!(!base.join("received").exists());
        });
    }
}
