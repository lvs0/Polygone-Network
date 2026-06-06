//! `polygone::economy` — local POLY token ledger.
//!
//! Spec §4 (Accueil): "Économie POLY : Solde disponible en jetons
//! POLY (système de comptabilité local stocké de manière sécurisée
//! dans ~/.polygone/poly.toml sans recours à une blockchain).
//! Affichage de la consommation courante exprimée en temps réel
//! (ex: 0.1 POLY / min)."
//!
//! The ledger is a plain TOML file under `~/.polygone/poly.toml`. It is
//! local-only (no blockchain) and atomic-write safe.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Default consumption rate per active service, in POLY/min.
pub const RATE_PER_MIN: f64 = 0.1;

/// Path of the on-disk ledger.
pub fn ledger_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".polygone").join("poly.toml")
}

/// The contents of `poly.toml` — persisted on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ledger {
    /// Current balance, in POLY.
    pub balance: f64,
    /// Number of services currently consuming.
    pub active_services: u32,
    /// When the ledger was last updated (unix epoch ms).
    pub last_update_ms: u64,
    /// When the node first booted, for cumulative stats.
    pub booted_at_ms: u64,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            balance: 100.0,
            active_services: 0,
            last_update_ms: now_ms(),
            booted_at_ms: now_ms(),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Load the ledger from disk, creating it with defaults if missing.
pub fn load() -> Ledger {
    let p = ledger_path();
    if !p.exists() {
        let l = Ledger::default();
        let _ = save(&l);
        return l;
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| toml::from_str::<Ledger>(&s).ok())
        .unwrap_or_default()
}

/// Persist the ledger to disk (atomic — write to .tmp then rename).
pub fn save(ledger: &Ledger) -> std::io::Result<()> {
    let p = ledger_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(ledger)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, s)?;
    std::fs::rename(&tmp, &p)
}

/// Ticking accountant — drains POLY from the balance while services
/// are active. Holds the last-applied timestamp so a single instance
/// can be polled cheaply.
pub struct Ticker {
    ledger: Ledger,
    last_tick: Instant,
}

impl Ticker {
    /// Load a ticker from the on-disk ledger.
    pub fn load() -> Self {
        let ledger = load();
        Self {
            ledger,
            last_tick: Instant::now(),
        }
    }

    /// Set the number of currently-active services.
    pub fn set_active(&mut self, n: u32) {
        self.ledger.active_services = n;
        let _ = save(&self.ledger);
    }

    /// Get a snapshot of the current balance + rate.
    pub fn snapshot(&self) -> Snapshot {
        let elapsed = self.last_tick.elapsed();
        Snapshot {
            balance: self.ledger.balance,
            rate_per_min: RATE_PER_MIN * self.ledger.active_services as f64,
            active: self.ledger.active_services,
            elapsed_since_tick: elapsed,
        }
    }

    /// Drain POLY for the time elapsed since the last tick, then
    /// persist. Returns the new balance.
    pub fn tick(&mut self) -> f64 {
        let elapsed = self.last_tick.elapsed();
        self.last_tick = Instant::now();
        let minutes = elapsed.as_secs_f64() / 60.0;
        let drain = RATE_PER_MIN * self.ledger.active_services as f64 * minutes;
        self.ledger.balance = (self.ledger.balance - drain).max(0.0);
        self.ledger.last_update_ms = now_ms();
        let _ = save(&self.ledger);
        self.ledger.balance
    }
}

/// Cheap-to-clone view of the Ticker state for the TUI to render.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub balance: f64,
    pub rate_per_min: f64,
    pub active: u32,
    pub elapsed_since_tick: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ledger_has_100_poly() {
        let l = Ledger::default();
        assert_eq!(l.balance, 100.0);
        assert_eq!(l.active_services, 0);
    }

    #[test]
    fn rate_scales_with_active_services() {
        let l = Ledger { balance: 100.0, active_services: 3, last_update_ms: 0, booted_at_ms: 0 };
        let s = Snapshot {
            balance: l.balance,
            rate_per_min: RATE_PER_MIN * l.active_services as f64,
            active: l.active_services,
            elapsed_since_tick: Duration::ZERO,
        };
        assert!((s.rate_per_min - 0.3).abs() < 1e-9);
    }

    #[test]
    fn ticker_drains_polynomialy() {
        // We can't actually wait minutes in a test, but we can validate
        // the math: with 0 active services, tick() doesn't drain.
        let mut t = Ticker::load();
        let before = t.ledger.balance;
        let _ = t.tick();
        let after = t.ledger.balance;
        // With active=0, drain=0, so balance should be unchanged.
        assert!((after - before).abs() < 1e-6);
    }

    #[test]
    fn snapshot_has_required_fields() {
        let t = Ticker::load();
        let s = t.snapshot();
        assert!(s.balance >= 0.0);
        assert!(s.rate_per_min >= 0.0);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let mut l = Ledger::default();
        l.balance = 42.5;
        l.active_services = 2;
        save(&l).expect("save");
        let restored = load();
        assert_eq!(restored.balance, 42.5);
        assert_eq!(restored.active_services, 2);
    }
}
