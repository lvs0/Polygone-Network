//! Policy module — allocation policy engines for polygoned
//!
//! Currently houses the GlowUp engine: deterministic, ML-free
//! resource allocation driven by tier + safety + real-time state.

pub mod glow_up;

// Re-exports for embeddable use (mirrors the types exposed via glow_up)
pub use glow_up::{
    GlowUpEngine,
};
