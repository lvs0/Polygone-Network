//! testutil — helpers partagés pour les tests du binaire client.
//!
//! `GLOBAL_STATE_LOCK` sérialise les tests qui touchent à l'état global du
//! process (ex. `std::env::set_var("HOME", …)` dans les tests duress)
//! contre les tests système qui dépendent de cet état (ex. le sandbox
//! systemd dans exec). Sans lui, deux tests parallèles se marchent
//! dessus de façon déterministe — la race HOME ↔ sandbox découverte le
//! 2026-08-08.

use std::sync::Mutex;

/// Verrou global des tests qui manipulent l'environnement du process.
pub(crate) static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());

/// Acquiert le verrou global d'état (traitement des locks empoisonnés).
pub(crate) fn with_global_state_guard() -> std::sync::MutexGuard<'static, ()> {
    GLOBAL_STATE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
