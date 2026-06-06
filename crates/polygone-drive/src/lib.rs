//! `polygone-drive` — distributed encrypted storage with
//! ephemeral public links and streaming.
//!
//! Spec §3: "Système de fichiers distribué chiffré … Liens
//! publics éphémères (expiration 24h) … Streaming média à la
//! volée sans téléchargement local."

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// File metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileMeta {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime: Option<String>,
    pub owner: String,
    pub path: PathBuf,
    pub uploaded_at_ms: u64,
}

/// Ephemeral public link (24h TTL by spec).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicLink {
    pub token: String,
    pub file_id: String,
    pub created_at_ms: u64,
    pub ttl: Duration,
}

/// Quota tracking.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Quota {
    pub used: u64,
    pub cap: u64,
}

impl Quota {
    /// 10 GiB default.
    pub fn default_10gib() -> Self {
        Self { used: 0, cap: 10 * 1024 * 1024 * 1024 }
    }
    /// Free bytes.
    pub fn free(&self) -> u64 { self.cap.saturating_sub(self.used) }
    /// Can `n` more bytes fit?
    pub fn can_fit(&self, n: u64) -> bool { n <= self.free() }
}

/// Streaming range request.
#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub start: u64,
    pub end: u64,
}

/// Local drive node.
pub struct DriveNode {
    files: HashMap<String, FileMeta>,
    links: HashMap<String, PublicLink>,
    quota: Quota,
    root: PathBuf,
}

impl DriveNode {
    /// Create a new node rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self {
            files: HashMap::new(),
            links: HashMap::new(),
            quota: Quota::default_10gib(),
            root,
        }
    }

    /// Store encrypted bytes. Returns the file id, or `None` if
    /// it would exceed quota or fail to write.
    pub fn put(
        &mut self,
        name: impl Into<String>,
        ciphertext: Vec<u8>,
        mime: Option<String>,
        owner: impl Into<String>,
    ) -> Option<String> {
        let size = ciphertext.len() as u64;
        if !self.quota.can_fit(size) { return None; }
        let mut h = Sha256::new(); h.update(&ciphertext);
        let id = format!("file:{}", hex::encode(&h.finalize()[..16]));
        let path = self.root.join(&id);
        std::fs::write(&path, &ciphertext).ok()?;
        let meta = FileMeta {
            id: id.clone(),
            name: name.into(),
            size,
            mime,
            owner: owner.into(),
            path,
            uploaded_at_ms: epoch_ms(),
        };
        self.files.insert(id.clone(), meta);
        self.quota.used += size;
        Some(id)
    }

    /// Get metadata.
    pub fn meta(&self, id: &str) -> Option<&FileMeta> { self.files.get(id) }

    /// Read a range (streaming).
    pub fn stream(&self, id: &str, r: Range) -> Option<Vec<u8>> {
        let m = self.files.get(id)?;
        if r.start >= m.size { return None; }
        let end = r.end.min(m.size - 1);
        let mut f = std::fs::File::open(&m.path).ok()?;
        f.seek(SeekFrom::Start(r.start)).ok()?;
        let mut buf = vec![0u8; (end - r.start + 1) as usize];
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    /// Issue a 24h public link.
    pub fn link(&mut self, file_id: &str) -> Option<String> {
        if !self.files.contains_key(file_id) { return None; }
        let mut h = Sha256::new();
        h.update(file_id.as_bytes());
        h.update(epoch_ms().to_be_bytes());
        let token = hex::encode(&h.finalize()[..16]);
        self.links.insert(token.clone(), PublicLink {
            token: token.clone(),
            file_id: file_id.into(),
            created_at_ms: epoch_ms(),
            ttl: Duration::from_secs(24 * 3600),
        });
        Some(token)
    }

    /// Resolve a token if not expired.
    pub fn resolve_link(&self, token: &str) -> Option<&str> {
        let l = self.links.get(token)?;
        if epoch_ms().saturating_sub(l.created_at_ms) > l.ttl.as_millis() as u64 {
            return None;
        }
        Some(&l.file_id)
    }

    /// Delete a file.
    pub fn delete(&mut self, id: &str) -> Option<u64> {
        let m = self.files.remove(id)?;
        let freed = m.size;
        let _ = std::fs::remove_file(&m.path);
        self.links.retain(|_, l| l.file_id != id);
        self.quota.used = self.quota.used.saturating_sub(freed);
        Some(freed)
    }

    /// Drop expired links.
    pub fn sweep_links(&mut self) -> usize {
        let now = epoch_ms();
        let before = self.links.len();
        self.links.retain(|_, l| now.saturating_sub(l.created_at_ms) <= l.ttl.as_millis() as u64);
        before - self.links.len()
    }

    /// Quota snapshot.
    pub fn quota(&self) -> Quota { self.quota }
    /// File count.
    pub fn len(&self) -> usize { self.files.len() }
}

fn epoch_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("polydrive-{name}-{}", epoch_ms()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn put_meta_roundtrip() {
        let mut n = DriveNode::new(root("p"));
        let id = n.put("a.txt", b"abc".to_vec(), Some("text/plain".into()), "o").expect("p");
        let m = n.meta(&id).expect("m");
        assert_eq!(m.size, 3);
    }

    #[test]
    fn stream_range() {
        let mut n = DriveNode::new(root("s"));
        let id = n.put("a", (0u8..=9).collect(), None, "o").expect("p");
        let b = n.stream(&id, Range { start: 3, end: 5 }).expect("s");
        assert_eq!(b, vec![3, 4, 5]);
    }

    #[test]
    fn link_resolves_within_24h() {
        let mut n = DriveNode::new(root("l"));
        let id = n.put("a", b"x".to_vec(), None, "o").expect("p");
        let t = n.link(&id).expect("l");
        assert_eq!(n.resolve_link(&t), Some(id.as_str()));
    }

    #[test]
    fn delete_invalidates_link() {
        let mut n = DriveNode::new(root("d"));
        let id = n.put("a", b"xx".to_vec(), None, "o").expect("p");
        let t = n.link(&id).expect("l");
        n.delete(&id).expect("d");
        assert!(n.resolve_link(&t).is_none());
    }

    #[test]
    fn quota_blocks_oversize() {
        let mut n = DriveNode::new(root("q"));
        n.quota = Quota { used: 0, cap: 5 };
        assert!(n.put("a", vec![0; 100], None, "o").is_none());
    }
}
