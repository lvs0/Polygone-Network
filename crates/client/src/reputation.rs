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
use std::path::{Path, PathBuf};

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
        let total = u64::from(self.ok) + u64::from(self.fail);
        if total == 0 {
            return 0; // unknown
        }
        ((u64::from(self.ok) * 100) / total) as u8
    }
}

/// The local reputation table.
pub const MAX_REPUTATION_NODES: usize = 4096;
const MAX_REPUTATION_FILE_BYTES: u64 = 16 * 1024 * 1024;

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

    /// Load the table (empty if absent). Oversized files are ignored rather
    /// than allowed to consume unbounded memory during startup.
    pub fn load() -> Self {
        Self::load_from_path(&Self::path())
    }

    fn load_from_path(path: &Path) -> Self {
        let Ok(metadata) = std::fs::metadata(path) else {
            return Self::default();
        };
        if metadata.len() > MAX_REPUTATION_FILE_BYTES {
            return Self::default();
        }
        let Ok(raw) = std::fs::read(path) else {
            return Self::default();
        };
        let Ok(mut table) = serde_json::from_slice::<Self>(&raw) else {
            return Self::default();
        };
        table.bound_loaded_nodes();
        table
    }

    fn bound_loaded_nodes(&mut self) {
        self.nodes.retain(|node, _| {
            !node.is_empty()
                && node.len() <= 64
                && node
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        });
        if self.nodes.len() <= MAX_REPUTATION_NODES {
            return;
        }
        let mut entries: Vec<_> = self.nodes.drain().collect();
        entries.sort_by(|(left_id, left), (right_id, right)| {
            let left_ts = left
                .last_ok_ts
                .unwrap_or(0)
                .max(left.last_fail_ts.unwrap_or(0));
            let right_ts = right
                .last_ok_ts
                .unwrap_or(0)
                .max(right.last_fail_ts.unwrap_or(0));
            right_ts.cmp(&left_ts).then_with(|| left_id.cmp(right_id))
        });
        entries.truncate(MAX_REPUTATION_NODES);
        self.nodes = entries.into_iter().collect();
    }

    /// Save atomically so a crash cannot truncate the previous local history.
    /// The temporary file is created beside the destination (same filesystem).
    fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
        let temporary =
            path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        let write_result = (|| {
            file.write_all(&raw)?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)?;
            Ok::<(), std::io::Error>(())
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        write_result?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// Record an outcome for a ghost node.
    pub fn record(&mut self, node: &str, ok: bool) {
        self.record_at_path(node, ok, &Self::path());
    }

    fn record_at_path(&mut self, node: &str, ok: bool, path: &Path) {
        if node.is_empty()
            || node.len() > 64
            || !node
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return;
        }
        if !self.nodes.contains_key(node) && self.nodes.len() >= MAX_REPUTATION_NODES {
            return; // bounded local history: never evict an existing peer's record
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let entry = self.nodes.entry(node.to_string()).or_default();
        if ok {
            entry.ok = entry.ok.saturating_add(1);
            entry.last_ok_ts = Some(now);
        } else {
            entry.fail = entry.fail.saturating_add(1);
            entry.last_fail_ts = Some(now);
        }
        let _ = self.save_to_path(path);
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
        let r = Reputation {
            ok: 3,
            fail: 1,
            ..Default::default()
        };
        assert_eq!(r.score(), 75);
    }

    #[test]
    fn unknown_node_scores_zero() {
        let t = ReputationTable::default();
        assert_eq!(t.score_of("ghost"), 0);
    }

    #[test]
    fn loaded_reputation_map_discards_invalid_ids_and_keeps_the_most_recent_entries() {
        let mut table = ReputationTable::default();
        table
            .nodes
            .insert("invalid id".into(), Reputation::default());
        table.nodes.insert("x".repeat(65), Reputation::default());
        for index in 0..(MAX_REPUTATION_NODES + 32) {
            table.nodes.insert(
                format!("peer-{index}"),
                Reputation {
                    ok: 1,
                    last_ok_ts: Some(index as u64),
                    ..Default::default()
                },
            );
        }
        table.bound_loaded_nodes();
        assert_eq!(table.nodes.len(), MAX_REPUTATION_NODES);
        assert!(!table.nodes.contains_key("invalid id"));
        assert!(!table.nodes.contains_key(&"x".repeat(65)));
        assert!(table
            .nodes
            .contains_key(&format!("peer-{}", MAX_REPUTATION_NODES + 31)));
        assert!(!table.nodes.contains_key("peer-0"));
    }

    #[test]
    fn score_does_not_overflow_at_maximum_counters() {
        let r = Reputation {
            ok: u32::MAX,
            fail: u32::MAX,
            ..Default::default()
        };
        assert_eq!(r.score(), 50);
    }

    #[test]
    fn load_and_record_use_an_isolated_atomic_file() {
        let directory = std::env::temp_dir().join(format!(
            "polygone-reputation-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("reputation.json");
        let mut table = ReputationTable::default();
        table.record_at_path("ghost-a", true, &path);
        table.record_at_path("ghost-a", false, &path);
        let loaded = ReputationTable::load_from_path(&path);
        assert_eq!(loaded.score_of("ghost-a"), 50);
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        assert!(std::fs::read_dir(&directory).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reputation_table_does_not_grow_past_its_peer_cap() {
        let directory = std::env::temp_dir().join(format!(
            "polygone-reputation-cap-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("reputation.json");
        let mut table = ReputationTable::default();
        for index in 0..MAX_REPUTATION_NODES {
            table
                .nodes
                .insert(format!("peer-{index}"), Reputation::default());
        }
        table.record_at_path("new-peer", true, &path);
        assert_eq!(table.nodes.len(), MAX_REPUTATION_NODES);
        assert!(!table.nodes.contains_key("new-peer"));
        table.nodes.get_mut("peer-0").unwrap().ok = u32::MAX;
        table.record_at_path("peer-0", true, &path);
        assert_eq!(table.nodes["peer-0"].ok, u32::MAX);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn record_updates_score() {
        let directory = std::env::temp_dir().join(format!(
            "polygone-reputation-score-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("reputation.json");
        let mut table = ReputationTable::default();
        table.record_at_path("ghost-a", true, &path);
        table.record_at_path("ghost-a", true, &path);
        table.record_at_path("ghost-a", false, &path);
        assert_eq!(table.score_of("ghost-a"), 66); // 2/3
        let loaded = ReputationTable::load_from_path(&path);
        assert_eq!(loaded.score_of("ghost-a"), 66);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
