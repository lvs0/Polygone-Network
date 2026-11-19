//! Apprentissage local du réseau.
//!
//! Chaque résultat observé met à jour une moyenne mobile exponentielle par
//! nœud (réussite, latence). Cette connaissance est **locale** : elle ne quitte
//! pas la machine et ne modifie aucune politique globale.
//!
//! Une connaissance **collective** n'est jamais appliquée automatiquement :
//! [`LearningStore::apply_collective`] exige une proposition explicitement
//! vérifiée par au moins deux nœuds. Une proposition non vérifiée est refusée.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Erreurs de la couche d'apprentissage.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LearningError {
    #[error("nœud vide")]
    EmptyNode,
    #[error("taux de réussite hors bornes : {0}")]
    SuccessOutOfRange(u16),
    #[error("proposition collective non vérifiée (au moins deux vérificateurs requis)")]
    Unverified,
    #[error("proposition sans échantillon")]
    NoSamples,
}

/// Santé observée d'un nœud.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeHealth {
    pub samples: u64,
    /// Taux de réussite lissé, en millièmes.
    pub success_permille: u16,
    /// Latence lissée, en millisecondes.
    pub latency_ms: u32,
}

impl Default for NodeHealth {
    fn default() -> Self {
        Self {
            samples: 0,
            success_permille: 1000,
            latency_ms: 0,
        }
    }
}

/// Connaissance locale, indexée par `node_id`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningStore {
    nodes: BTreeMap<String, NodeHealth>,
}

impl LearningStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre une observation. EWMA 7/8 : une anomalie isolée ne renverse
    /// pas une réputation établie, une panne répétée la fait baisser.
    pub fn observe(&mut self, node: &str, success: bool, latency_ms: u64) {
        let entry = self.nodes.entry(node.to_string()).or_default();
        let sample = if success { 1000u32 } else { 0 };
        let latency = latency_ms.min(u64::from(u32::MAX)) as u32;
        if entry.samples == 0 {
            entry.success_permille = sample as u16;
            entry.latency_ms = latency;
        } else {
            entry.success_permille = ((u32::from(entry.success_permille) * 7 + sample) / 8) as u16;
            entry.latency_ms = ((u64::from(entry.latency_ms) * 7 + u64::from(latency)) / 8) as u32;
        }
        entry.samples = entry.samples.saturating_add(1);
    }

    /// Taux d'échec lissé, ou la valeur déclarée si le nœud est inconnu.
    pub fn failure_permille(&self, node: &str, declared: u16) -> u16 {
        match self.nodes.get(node) {
            Some(health) if health.samples > 0 => 1000u16.saturating_sub(health.success_permille),
            _ => declared,
        }
    }

    /// Taux de réussite lissé, ou `default` si le nœud est inconnu.
    pub fn success_permille(&self, node: &str, default: u16) -> u16 {
        self.nodes
            .get(node)
            .filter(|health| health.samples > 0)
            .map(|health| health.success_permille)
            .unwrap_or(default)
    }

    pub fn health(&self, node: &str) -> Option<&NodeHealth> {
        self.nodes.get(node)
    }

    pub fn nodes(&self) -> &BTreeMap<String, NodeHealth> {
        &self.nodes
    }

    /// Applique une proposition collective **déjà vérifiée**. Cette méthode
    /// n'est appelée par aucun chemin automatique : c'est le point de contrôle
    /// explicite entre apprentissage local et connaissances collectives.
    pub fn apply_collective(&mut self, proposal: &CollectiveProposal) -> Result<(), LearningError> {
        if proposal.node.trim().is_empty() {
            return Err(LearningError::EmptyNode);
        }
        if proposal.success_permille > 1000 {
            return Err(LearningError::SuccessOutOfRange(proposal.success_permille));
        }
        if proposal.samples == 0 {
            return Err(LearningError::NoSamples);
        }
        let mut verifiers = proposal.verified_by.clone();
        verifiers.sort();
        verifiers.dedup();
        if verifiers.len() < 2 {
            return Err(LearningError::Unverified);
        }
        self.nodes.insert(
            proposal.node.clone(),
            NodeHealth {
                samples: proposal.samples,
                success_permille: proposal.success_permille,
                latency_ms: proposal.latency_ms,
            },
        );
        Ok(())
    }
}

/// Connaissance proposée par le réseau, jamais appliquée seule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveProposal {
    pub node: String,
    pub success_permille: u16,
    pub latency_ms: u32,
    pub samples: u64,
    pub verified_by: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failures_lower_the_score() {
        let mut store = LearningStore::new();
        for _ in 0..12 {
            store.observe("node-a", false, 40);
        }
        assert!(
            store.failure_permille("node-a", 0) > 700,
            "un nœud qui échoue doit devenir risqué"
        );
    }

    #[test]
    fn a_single_failure_does_not_erase_a_good_history() {
        let mut store = LearningStore::new();
        for _ in 0..20 {
            store.observe("node-a", true, 10);
        }
        store.observe("node-a", false, 10);
        assert!(store.success_permille("node-a", 1000) > 800);
    }

    #[test]
    fn unknown_nodes_fall_back_to_the_declared_rate() {
        let store = LearningStore::new();
        assert_eq!(store.failure_permille("unknown", 250), 250);
        assert_eq!(store.success_permille("unknown", 750), 750);
    }

    #[test]
    fn an_unverified_collective_proposal_is_refused() {
        let mut store = LearningStore::new();
        let proposal = CollectiveProposal {
            node: "node-a".to_string(),
            success_permille: 1000,
            latency_ms: 1,
            samples: 50,
            verified_by: vec!["only-one".to_string()],
        };
        assert_eq!(
            store.apply_collective(&proposal),
            Err(LearningError::Unverified)
        );
        assert!(store.health("node-a").is_none());
    }

    #[test]
    fn a_verified_collective_proposal_is_applied_explicitly() {
        let mut store = LearningStore::new();
        let proposal = CollectiveProposal {
            node: "node-a".to_string(),
            success_permille: 900,
            latency_ms: 12,
            samples: 50,
            verified_by: vec!["v1".to_string(), "v2".to_string(), "v1".to_string()],
        };
        store.apply_collective(&proposal).expect("vérifiée");
        assert_eq!(store.success_permille("node-a", 0), 900);
    }
}
