//! `polygone-brain` — local AI inference with multi-model routing.
//!
//! Spec §3: "Module d'intelligence artificielle locale … optimisé
//! pour l'inférence sur CPU … Système de calcul distribué (Petals)
//! pour décharger les calculs intensifs."

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Which model to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelKind {
    /// Local Notch SLM (Qwen2.5 1.5B).
    Notch,
    /// Petals — distributed mesh inference.
    Petals,
    /// Local Ollama.
    Ollama,
}

impl ModelKind {
    /// Tokens/s estimate.
    pub fn tok_per_sec(&self) -> f32 {
        match self {
            Self::Notch  => 8.0,
            Self::Petals => 25.0,
            Self::Ollama => 4.0,
        }
    }
}

/// An inference request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inference {
    pub id: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub model: ModelKind,
    pub created_at_ms: u64,
}

/// A finished inference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceResult {
    pub inference_id: String,
    pub model: ModelKind,
    pub output: String,
    pub tokens: u32,
    pub duration: Duration,
    pub poly_cost: f32,
}

/// Brain stats.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct BrainStats {
    pub inferences: u64,
    pub tokens: u64,
    pub poly_spent: f32,
    pub poly_earned: f32,
}

/// Local brain node serving Notch/Petals/Ollama.
pub struct Brain {
    inflight: HashMap<String, Inference>,
    results: Vec<InferenceResult>,
    stats: BrainStats,
    serves_petals: bool,
}

impl Brain {
    pub fn new() -> Self {
        Self { inflight: HashMap::new(), results: Vec::new(), stats: BrainStats::default(), serves_petals: false }
    }

    /// Enable Petals shard mode (earn POLY by serving the mesh).
    pub fn set_serves_petals(&mut self, yes: bool) { self.serves_petals = yes; }

    /// Submit an inference request. Returns the id.
    pub fn submit(&mut self, inf: Inference) -> String {
        let id = inf.id.clone();
        self.inflight.insert(id.clone(), inf);
        id
    }

    /// Mark inference as completed with generated output.
    pub fn complete(&mut self, inference_id: &str, output: String) -> Option<InferenceResult> {
        let inf = self.inflight.remove(inference_id)?;
        let now = epoch_ms();
        let dur = Duration::from_millis(now.saturating_sub(inf.created_at_ms));
        let tokens = (output.len() as u32).div_ceil(4); // cheap estimate
        let poly_cost = if inf.model == ModelKind::Petals && self.serves_petals {
            -(tokens as f32) * 0.0001
        } else {
            (tokens as f32) * 0.001
        };
        if poly_cost < 0.0 { self.stats.poly_earned += -poly_cost; }
        else { self.stats.poly_spent += poly_cost; }
        self.stats.inferences += 1;
        self.stats.tokens += tokens as u64;
        let res = InferenceResult {
            inference_id: inference_id.into(), model: inf.model, output,
            tokens, duration: dur, poly_cost,
        };
        if self.results.len() >= 100 { self.results.remove(0); }
        self.results.push(res.clone());
        Some(res)
    }

    /// Last N results (most recent first).
    pub fn recent(&self, n: usize) -> &[InferenceResult] {
        let start = self.results.len().saturating_sub(n);
        &self.results[start..]
    }

    /// Stats.
    pub fn stats(&self) -> BrainStats { self.stats }
}

impl Default for Brain { fn default() -> Self { Self::new() } }

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inf(id: &str, model: ModelKind) -> Inference {
        Inference { id: id.into(), prompt: "hi".into(), max_tokens: 128, model, created_at_ms: epoch_ms() }
    }

    #[test]
    fn notch_costs_poly() {
        let mut b = Brain::new();
        let id = b.submit(inf("i1", ModelKind::Notch));
        let r = b.complete(&id, "hello".into()).unwrap();
        assert!(r.poly_cost > 0.0);
        assert_eq!(b.stats().inferences, 1);
    }

    #[test]
    fn petals_earns_as_shard() {
        let mut b = Brain::new();
        b.set_serves_petals(true);
        let id = b.submit(inf("i1", ModelKind::Petals));
        let r = b.complete(&id, "x".into()).unwrap();
        assert!(r.poly_cost < 0.0);
        assert!(b.stats().poly_earned > 0.0);
    }

    #[test]
    fn ollama_local_costs() {
        let mut b = Brain::new();
        let id = b.submit(inf("i1", ModelKind::Ollama));
        let r = b.complete(&id, "ok".into()).unwrap();
        assert!(r.poly_cost > 0.0);
    }

    #[test]
    fn recent_caps_at_100() {
        let mut b = Brain::new();
        for i in 0..150u32 {
            let id = b.submit(inf(&format!("i{i}"), ModelKind::Notch));
            b.complete(&id, "x".into());
        }
        assert_eq!(b.recent(200).len(), 100);
        assert_eq!(b.stats().inferences, 150);
    }

    #[test]
    fn petal_throughput_higher() {
        assert!(ModelKind::Petals.tok_per_sec() > ModelKind::Notch.tok_per_sec());
        assert!(ModelKind::Notch.tok_per_sec() > ModelKind::Ollama.tok_per_sec());
    }
}