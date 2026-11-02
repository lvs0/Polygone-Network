//! Windows resource discovery — placeholder pour un futur portage.
//!
//! VÉRITÉ (2026-08-08) : `WindowsPlatform` n'est pas défini. Un build
//! Windows de `polygoned` échoue à la compilation avec un message
//! explicite (`compile_error!` dans resources/mod.rs) — jamais un lien
//! cassé qui prétendrait fonctionner.
//!
//! La promesse produit est Linux + macOS (README) ; Windows rejoint la
//! table des lacunes ARCHITECTURE.md §11. Ce fichier est le point
//! d'entrée du futur portage : `pub struct WindowsPlatform` + impl
//! `Platform` (découverte de ressources), testée sur un runner Windows.
