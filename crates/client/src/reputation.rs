//! reputation — trust layer for RES ghost nodes.
//!
//! Each execution outcome (grant received / timeout / failure) is recorded
//! per ghost node in `~/.polygone/reputation.json`. The score lets a borrower
//! prefer ghosts that actually deliver.
//!
//! Honest scope: local, per-machine history — no global identity, no proof
//! of work. Production (Phase 8+) would add verifiable receipts (ML-DSA
//! signed grants) and a network-level reputation exchange.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A ghost's execution history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reputation {
    pub ok: u32,
    pub fail: u32,
    #[serde(default)]
    pub last_ok_ts: Option<u64>,
    #[serde(default)]
    pub last_fail_ts: Option<u64>,
}

impl Reputation {
    /// Success ratio as a percentage (100 = flawless).
    pub fn score(&self) -> u8 {
        let total = self.ok + self.fail;
        if total == 0 {
            return 0; // unknown
        }
        ((self.ok as f32 / total as f32) * 100.0) as u8
    }
}

/// The local reputation table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReputationTable {
    pub nodes: HashMap<String, Reputation>,
}

impl ReputationTable {
    /// Where the table lives.
    pub fn path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".polygone").join("reputation.json")
    }

    /// Load the table (empty if absent).
    pub fn load() -> Self {
        let path = Self::path();
        if !path.exists() {
            return Self::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Save the table (best-effort, perms 600).
    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, raw);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }

    /// Record an outcome for a ghost node.
    pub fn record(&mut self, node: &str, ok: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let entry = self.nodes.entry(node.to_string()).or_default();
        if ok {
            entry.ok += 1;
            entry.last_ok_ts = Some(now);
        } else {
            entry.fail += 1;
            entry.last_fail_ts = Some(now);
        }
        self.save();
    }

    /// The score of a node (0 if unknown).
    pub fn score_of(&self, node: &str) -> u8 {
        self.nodes.get(node).map(|r| r.score()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_is_percentage() {
        let mut r = Reputation::default();
        r.ok = 3;
        r.fail = 1;
        assert_eq!(r.score(), 75);
    }

    #[test]
    fn unknown_node_scores_zero() {
        let t = ReputationTable::default();
        assert_eq!(t.score_of("ghost"), 0);
    }

    #[test]
    fn record_updates_score() {
        let mut t = ReputationTable::default();
        t.record("ghost-a", true);
        t.record("ghost-a", true);
        t.record("ghost-a", false);
        assert_eq!(t.score_of("ghost-a"), 66); // 2/3
        t.save();
        // Round-trips.
        let t2 = ReputationTable::load();
        assert_eq!(t2.score_of("ghost-a"), 66);
    }
}
