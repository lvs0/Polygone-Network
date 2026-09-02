//! Polygone Drive — file transfer service.
//!
//! MVP: simple send/receive over relay or mesh, stored in `~/.polygone/received/`.

use polygone_core::NodeId;
use crate::Result;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct DriveFile {
    pub name: String,
    pub size: u64,
    pub sender: NodeId,
    pub path: PathBuf,
}

impl DriveFile {
    /// Default received directory.
    pub fn received_dir() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".polygone")
            .join("received")
    }

    /// Save a received file to the received directory.
    pub fn save(sender: &NodeId, filename: &str, data: &[u8]) -> Result<Self> {
        let dir = Self::received_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(filename);
        std::fs::write(&path, data)?;
        Ok(Self {
            name: filename.to_string(),
            size: data.len() as u64,
            sender: sender.clone(),
            path,
        })
    }

    /// List received files.
    pub fn list() -> Result<Vec<Self>> {
        let dir = Self::received_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let size = entry.metadata()?.len();
                files.push(Self {
                    name,
                    size,
                    sender: NodeId([0u8; 16]),
                    path,
                });
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }
}
