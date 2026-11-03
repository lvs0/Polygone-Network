//! Fenêtre bibliothèque de `polygone-relay`.
//!
//! Le binaire reste l'interface ; cette fenêtre existe pour qu'un test puisse
//! démarrer **le vrai relais** sur une socket éphémère et lui faire traverser
//! de vraies trames, au lieu de simuler son comportement. Un test qui réécrit
//! la logique du relais ne prouve rien du relais.

pub mod relay;
