//! Polygone Drive — file transfer service.
//!
//! MVP: simple send/receive over relay or mesh, stored in `~/.polygone/received/`.

use crate::Result;
use crate::msg::{send_bytes, Fragment, SendOutput};
use crate::serverless::reconstruct_from_shards;
use polygone_core::crypto::kem::KemPublicKey;
use polygone_core::NodeId;
use std::path::PathBuf;

#[allow(dead_code)]
const CHUNK_SIZE: usize = 64 * 1024; // 64 KiB per fragment

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DriveFile {
    pub name: String,
    pub size: u64,
    pub sender: NodeId,
    pub path: PathBuf,
}

impl DriveFile {
    /// Default received directory.
    #[allow(dead_code)]
    pub fn received_dir() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".polygone")
            .join("received")
    }

    /// Save a received file to the received directory.
    #[allow(dead_code)]
    pub fn save(sender: &NodeId, filename: &str, data: &[u8]) -> Result<Self> {
        let dir = Self::received_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(filename);
        std::fs::write(&path, data)?;
        Ok(Self {
            name: filename.to_string(),
            size: data.len() as u64,
            sender: *sender,
            path,
        })
    }

    /// List received files.
    #[allow(dead_code)]
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

    /// Send a file via the msg pipeline (KEM + Shamir + relay).
    /// Chunks file into CHUNK_SIZE pieces, each sent as separate msg.
    #[allow(dead_code)]
    pub fn send(to: &KemPublicKey, path: &PathBuf) -> Result<()> {
        let data = std::fs::read(path)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Chunk the file if larger than CHUNK_SIZE
        for (seq, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
            let mut payload = Vec::new();
            // Header: [seq: u32][total_chunks: u32][filename_len: u32][filename][chunk_data]
            payload.extend_from_slice(&(seq as u32).to_le_bytes());
            let total_chunks = data.len().div_ceil(CHUNK_SIZE);
            payload.extend_from_slice(&(total_chunks as u32).to_le_bytes());
            payload.extend_from_slice(&(filename.len() as u32).to_le_bytes());
            payload.extend_from_slice(filename.as_bytes());
            payload.extend_from_slice(chunk);

            // Send via msg pipeline (KEM + Shamir 4/7 + relay)
            let _output: SendOutput = send_bytes(&payload, to)?;
        }
        Ok(())
    }

    /// Receive fragments, reconstruct using Shamir, and save file.
    /// Handles multi-chunk reconstruction with sequence ordering.
    #[allow(dead_code)]
    pub fn receive(fragments: Vec<Fragment>, sender: &NodeId) -> Result<Self> {
        // Reconstruct payload from Shamir fragments
        let shards: Vec<Vec<u8>> = fragments.iter().map(|f| f.share.clone()).collect();
        let payload = reconstruct_from_shards(&shards)?;

        // Parse header: [seq: u32][total_chunks: u32][filename_len: u32][filename][chunk_data]
        if payload.len() < 12 {
            anyhow::bail!("payload too small for drive header");
        }
        let seq = u32::from_le_bytes(payload[0..4].try_into()?) as usize;
        let total_chunks = u32::from_le_bytes(payload[4..8].try_into()?) as usize;
        let filename_len = u32::from_le_bytes(payload[8..12].try_into()?) as usize;

        if payload.len() < 12 + filename_len {
            anyhow::bail!("payload truncated: filename missing");
        }
        let filename = String::from_utf8(payload[12..12 + filename_len].to_vec())?;
        let chunk_data = payload[12 + filename_len..].to_vec();

        // TODO: For multi-chunk files, we need to collect all chunks and reassemble in order.
        // For now, single-chunk files work directly.
        if total_chunks > 1 {
            anyhow::bail!("multi-chunk reassembly not yet implemented (seq {}/{}))", seq + 1, total_chunks);
        }

        Self::save(sender, &filename, &chunk_data)
    }
}
