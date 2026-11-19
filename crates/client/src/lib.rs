//! Fenêtre publique de `polygone-client`.
//!
//! Le binaire `polygone` (`src/main.rs`) est l'interface utilisateur ; ce qui
//! est exposé **ici** est ce qu'un autre programme peut réutiliser.
//!
//! - [`blocks`] — le magasin de blocs adressé par BLAKE3 (incrément 2b-12b), le
//!   premier lien pair-à-pair direct du projet.
//! - [`mesh`] — la découverte des pairs du LAN (UDP `7642`). Monté ici à
//!   l'incrément 2b-13 parce que `node status` doit **mesurer** le mesh, pas le
//!   supposer : le binaire s'y branche par `use polygone_client::mesh;`.
//! - [`node`] — le service de blocs qui obtient un bail du daemon, sert sous
//!   ce bail et s'arrête sur signal, perte du daemon ou retrait de politique.
//! - [`calc`] — le cache de calcul : la clé porte la **version du calculateur**
//!   (`BLAKE3(version ‖ entrée)`), le résultat est un bloc vérifié par son hash.
//!
//! Le sous-système compute/réseau (`identity`, `msg`, `exec`, `net`,
//! `reputation`) est exposé ici depuis l'incrément « prototype complet »
//! (2026-11-19) : le gateway réutilise le chemin d'exécution distante **prouvé**
//! sur deux hôtes (`net::borrow_compute`, preuve P0-DEUX-HOTES-2026-11-19)
//! sans le réécrire. Le binaire `polygone` garde ses copies privées ; aucun
//! comportement du binaire ne change. `tui` et `hide` restent dans le binaire.
//!
//! `testutil` (verrou global d'état des tests) est exposé en `cfg(test)` pour
//! que les sous-modules de test de `net`/`exec` compilent aussi côté lib.

pub mod blocks;
pub mod calc;
pub mod ecosystem;
pub mod exec;
pub mod identity;
pub mod mesh;
pub mod msg;
pub mod net;
pub mod node;
pub mod node_isolation;
pub mod pause;
pub mod profile;
pub mod refusal;
pub mod reputation;
pub mod uninstall;

#[cfg(test)]
mod testutil;
