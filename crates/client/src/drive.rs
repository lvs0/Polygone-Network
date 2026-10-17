//! Polygone Drive — encrypted file transfer helpers.
//!
//! Each chunk is an independent message-pipeline output. The caller transports
//! those outputs and passes them back together to `receive_chunks`.

use crate::msg::{send_bytes, SendOutput};
use crate::Result;
use anyhow::Context;
use polygone_core::crypto::kem::{KemPublicKey, KemSecretKey};
use polygone_core::NodeId;
use rand::RngCore;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// 12 KiB plaintext chunks leave room for Shamir, JSON byte-array encoding,
/// and envelope metadata under the relay's 64 KiB line cap.
pub(crate) const CHUNK_SIZE: usize = 12 * 1024;
/// Largest file accepted by the network transfer path (256 MiB).
pub(crate) const MAX_TRANSFER_BYTES: usize = 256 * 1024 * 1024;
const CHUNK_MAGIC: &[u8; 8] = b"PGDRIVE1";
const TRANSFER_ID_SIZE: usize = 16;
const CHUNK_HEADER_SIZE: usize = CHUNK_MAGIC.len() + TRANSFER_ID_SIZE + 12;
const RECEIVED_TEMP_PREFIX: &str = ".polygone-received-";
static NEXT_RECEIVED_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DriveFile {
    pub name: String,
    pub size: u64,
    pub sender: NodeId,
    pub path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct DecodedChunk {
    pub(crate) transfer_id: [u8; TRANSFER_ID_SIZE],
    pub(crate) seq: usize,
    pub(crate) total: usize,
    pub(crate) filename: String,
    pub(crate) data: Vec<u8>,
}

type ChunkPreparer<'a> = dyn FnMut(&[u8], &str, &KemPublicKey, [u8; TRANSFER_ID_SIZE], usize, u32) -> Result<SendOutput>
    + 'a;

struct PreparedSendChunks<'a> {
    data: &'a [u8],
    filename: String,
    recipient: &'a KemPublicKey,
    transfer_id: [u8; TRANSFER_ID_SIZE],
    total_chunks: u32,
    next_seq: usize,
    prepare_chunk: Box<ChunkPreparer<'a>>,
}

impl Iterator for PreparedSendChunks<'_> {
    type Item = Result<SendOutput>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_seq >= self.total_chunks as usize {
            return None;
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        let start = seq * CHUNK_SIZE;
        let end = start.saturating_add(CHUNK_SIZE).min(self.data.len());
        let result = (self.prepare_chunk)(
            &self.data[start..end],
            &self.filename,
            self.recipient,
            self.transfer_id,
            seq,
            self.total_chunks,
        );
        if result.is_err() {
            // An error is terminal: never attempt or emit a later block.
            self.next_seq = self.total_chunks as usize;
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.total_chunks as usize).saturating_sub(self.next_seq);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PreparedSendChunks<'_> {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileSnapshot {
    len: u64,
    modified: SystemTime,
}

trait SendFileReader: Read {
    fn snapshot(&self) -> io::Result<FileSnapshot>;
}

impl SendFileReader for File {
    fn snapshot(&self) -> io::Result<FileSnapshot> {
        let metadata = self.metadata()?;
        Ok(FileSnapshot {
            len: metadata.len(),
            modified: metadata.modified()?,
        })
    }
}

struct SendFileChunks<'a, R> {
    file: R,
    filename: String,
    recipient: &'a KemPublicKey,
    transfer_id: [u8; TRANSFER_ID_SIZE],
    snapshot: FileSnapshot,
    total_chunks: u32,
    next_seq: usize,
}

impl<R: SendFileReader> Iterator for SendFileChunks<'_, R> {
    type Item = Result<SendOutput>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_seq >= self.total_chunks as usize {
            return None;
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        let result = self.prepare_next(seq);
        if result.is_err() {
            // A read, mutation, or encryption error is terminal.
            self.next_seq = self.total_chunks as usize;
        }
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.total_chunks as usize).saturating_sub(self.next_seq);
        (remaining, Some(remaining))
    }
}

impl<R: SendFileReader> SendFileChunks<'_, R> {
    fn prepare_next(&mut self, seq: usize) -> Result<SendOutput> {
        validate_file_snapshot(&self.file, &self.snapshot)?;

        let file_size = usize::try_from(self.snapshot.len)?;
        let start = seq * CHUNK_SIZE;
        let chunk_size = file_size.saturating_sub(start).min(CHUNK_SIZE);
        let mut data = vec![0; chunk_size];
        match self.file.read_exact(&mut data) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                anyhow::bail!("fichier tronqué pendant le flux d'envoi Drive");
            }
            Err(error) => {
                return Err(error).context("erreur pendant la lecture du fichier Drive");
            }
        }
        validate_file_snapshot(&self.file, &self.snapshot)?;

        if seq + 1 == self.total_chunks as usize {
            let mut extra = [0u8; 1];
            match self.file.read(&mut extra) {
                Ok(0) => {}
                Ok(_) => anyhow::bail!("fichier agrandi pendant le flux d'envoi Drive"),
                Err(error) => {
                    return Err(error)
                        .context("erreur pendant la vérification finale du fichier Drive");
                }
            }
            validate_file_snapshot(&self.file, &self.snapshot)?;
        }

        DriveFile::prepare_send_chunk(
            &data,
            &self.filename,
            self.recipient,
            self.transfer_id,
            seq,
            self.total_chunks,
        )
    }
}

impl<R: SendFileReader> ExactSizeIterator for SendFileChunks<'_, R> {}

fn validate_file_snapshot<R: SendFileReader>(file: &R, expected: &FileSnapshot) -> Result<()> {
    let current = file
        .snapshot()
        .context("impossible de vérifier l'état du fichier Drive pendant l'envoi")?;
    if current.len < expected.len {
        anyhow::bail!("fichier tronqué pendant le flux d'envoi Drive");
    }
    if current.len > expected.len {
        if current.len > MAX_TRANSFER_BYTES as u64 {
            anyhow::bail!(
                "fichier agrandi au-delà de la limite 256 Mio pendant le flux d'envoi Drive"
            );
        }
        anyhow::bail!("fichier agrandi pendant le flux d'envoi Drive");
    }
    if current.modified != expected.modified {
        anyhow::bail!("fichier modifié pendant le flux d'envoi Drive");
    }
    Ok(())
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
        Self::save_in(&Self::received_dir(), sender, filename, data)
    }

    fn save_in(dir: &Path, sender: &NodeId, filename: &str, data: &[u8]) -> Result<Self> {
        let (name, path) = save_received_in(dir, filename, data)?;
        Ok(Self {
            name,
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
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            if name.starts_with(RECEIVED_TEMP_PREFIX) {
                continue;
            }
            if path.is_file() {
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

    /// Prepare independently encrypted chunks for transport to a recipient.
    /// Empty files are represented by one header-only chunk.
    ///
    /// This batch API reads the bounded input and collects every encrypted
    /// output through [`Self::prepare_send`]. Prefer [`Self::prepare_send_iter`]
    /// when the consumer can process and drop each output before requesting the
    /// next one; otherwise memory use grows with the number of chunks.
    #[allow(dead_code)]
    pub fn send(to: &KemPublicKey, path: &Path) -> Result<Vec<SendOutput>> {
        let data = read_file_capped(path, MAX_TRANSFER_BYTES)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        Self::prepare_send(&data, filename, to)
    }

    /// Read and encrypt a file lazily, producing one output per iterator step.
    ///
    /// The initial file size is checked against the 256 MiB limit before the
    /// iterator is returned. Each step reads only one plaintext block, then
    /// checks the open file's length and modification timestamp; the final
    /// step also probes for unexpected extra bytes. Truncation, growth,
    /// metadata-visible modification, read errors, and encryption errors are
    /// yielded once and terminate iteration. Outputs yielded before a later
    /// error remain the caller's responsibility to discard/abort in transit.
    /// This is not an immutable filesystem snapshot: an in-place rewrite that
    /// preserves size and is not reflected by the filesystem timestamp may not
    /// be detected.
    #[allow(dead_code)]
    pub fn send_iter<'a>(
        to: &'a KemPublicKey,
        path: &Path,
    ) -> Result<impl ExactSizeIterator<Item = Result<SendOutput>> + 'a> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            anyhow::bail!("la source Drive n'est pas un fichier régulier");
        }
        let snapshot = FileSnapshot {
            len: metadata.len(),
            modified: metadata.modified()?,
        };
        let size = usize::try_from(snapshot.len)
            .map_err(|_| anyhow::anyhow!("fichier trop volumineux pour ce système"))?;
        check_transfer_size(size)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");
        let filename = safe_filename(filename)?;
        u32::try_from(filename.len()).map_err(|_| anyhow::anyhow!("nom de fichier trop long"))?;
        let total_chunks = size.max(1).div_ceil(CHUNK_SIZE);
        let total_chunks = u32::try_from(total_chunks)
            .map_err(|_| anyhow::anyhow!("fichier trop volumineux pour le format Drive"))?;

        Ok(SendFileChunks {
            file,
            filename,
            recipient: to,
            transfer_id: Self::new_transfer_id(),
            snapshot,
            total_chunks,
            next_seq: 0,
        })
    }

    /// Encrypt file bytes as ordered, independently authenticated message chunks.
    ///
    /// This compatibility API collects every encrypted output in memory. Prefer
    /// [`Self::prepare_send_iter`] when the consumer can process one output at a
    /// time; at the 256 MiB input cap, the batch outputs are estimated at about
    /// 2.25–2.30 GiB before allocator overhead.
    #[allow(dead_code)]
    pub fn prepare_send(data: &[u8], filename: &str, to: &KemPublicKey) -> Result<Vec<SendOutput>> {
        let transfer_id = Self::new_transfer_id();
        Self::prepare_send_with_id(data, filename, to, transfer_id)
    }

    /// Prepare encrypted chunks lazily, yielding one output per `next()` call.
    ///
    /// The 256 MiB input limit, filename, and wire chunk count are validated
    /// before the iterator is returned, so no output is emitted for invalid
    /// input. Blocks retain their stable sequence and transfer id. If preparing
    /// a block fails, that error is yielded once and iteration then ends. To
    /// realize the memory benefit, consume and drop each output before calling
    /// `next()` again; collecting the iterator recreates batch-like retention.
    #[allow(dead_code)]
    pub fn prepare_send_iter<'a>(
        data: &'a [u8],
        filename: &str,
        to: &'a KemPublicKey,
    ) -> Result<impl ExactSizeIterator<Item = Result<SendOutput>> + 'a> {
        Self::prepare_send_iter_with_id(data, filename, to, Self::new_transfer_id())
    }

    pub(crate) fn new_transfer_id() -> [u8; TRANSFER_ID_SIZE] {
        let mut transfer_id = [0u8; TRANSFER_ID_SIZE];
        rand::rngs::OsRng.fill_bytes(&mut transfer_id);
        transfer_id
    }

    fn prepare_send_with_id(
        data: &[u8],
        filename: &str,
        to: &KemPublicKey,
        transfer_id: [u8; 16],
    ) -> Result<Vec<SendOutput>> {
        Self::prepare_send_iter_with_id(data, filename, to, transfer_id)?.collect()
    }

    fn prepare_send_iter_with_id<'a>(
        data: &'a [u8],
        filename: &str,
        to: &'a KemPublicKey,
        transfer_id: [u8; TRANSFER_ID_SIZE],
    ) -> Result<PreparedSendChunks<'a>> {
        Self::prepare_send_iter_with_preparer(
            data,
            filename,
            to,
            transfer_id,
            Box::new(Self::prepare_send_chunk),
        )
    }

    fn prepare_send_iter_with_preparer<'a>(
        data: &'a [u8],
        filename: &str,
        to: &'a KemPublicKey,
        transfer_id: [u8; TRANSFER_ID_SIZE],
        prepare_chunk: Box<ChunkPreparer<'a>>,
    ) -> Result<PreparedSendChunks<'a>> {
        check_transfer_size(data.len())?;
        let filename = safe_filename(filename)?;
        u32::try_from(filename.len()).map_err(|_| anyhow::anyhow!("nom de fichier trop long"))?;
        let total_chunks = data.len().max(1).div_ceil(CHUNK_SIZE);
        let total_chunks_wire = u32::try_from(total_chunks)
            .map_err(|_| anyhow::anyhow!("fichier trop volumineux pour le format Drive"))?;
        Ok(PreparedSendChunks {
            data,
            filename,
            recipient: to,
            transfer_id,
            total_chunks: total_chunks_wire,
            next_seq: 0,
            prepare_chunk,
        })
    }

    pub(crate) fn prepare_send_chunk(
        data: &[u8],
        filename: &str,
        to: &KemPublicKey,
        transfer_id: [u8; TRANSFER_ID_SIZE],
        seq: usize,
        total: u32,
    ) -> Result<SendOutput> {
        let filename = safe_filename(filename)?;
        let filename_bytes = filename.as_bytes();
        let filename_len = u32::try_from(filename_bytes.len())
            .map_err(|_| anyhow::anyhow!("nom de fichier trop long"))?;
        let total = usize::try_from(total)?;
        if total == 0 || seq >= total {
            anyhow::bail!("séquence de bloc Drive invalide ({seq}/{total})");
        }
        let is_final = seq + 1 == total;
        if data.len() > CHUNK_SIZE
            || (!is_final && data.len() != CHUNK_SIZE)
            || (is_final && total > 1 && data.is_empty())
        {
            anyhow::bail!("taille de bloc Drive invalide à la séquence {seq}");
        }

        let mut payload = Vec::with_capacity(CHUNK_HEADER_SIZE + filename_bytes.len() + data.len());
        payload.extend_from_slice(CHUNK_MAGIC);
        payload.extend_from_slice(&transfer_id);
        payload.extend_from_slice(&u32::try_from(seq)?.to_le_bytes());
        payload.extend_from_slice(&u32::try_from(total)?.to_le_bytes());
        payload.extend_from_slice(&filename_len.to_le_bytes());
        payload.extend_from_slice(filename_bytes);
        payload.extend_from_slice(data);
        send_bytes(&payload, to)
    }

    pub(crate) fn is_network_chunk(payload: &[u8]) -> bool {
        payload.starts_with(CHUNK_MAGIC)
    }

    pub(crate) fn decode_network_chunk(payload: &[u8]) -> Result<DecodedChunk> {
        decode_chunk(payload)
    }

    /// Decrypt and save one single-chunk file.
    #[allow(dead_code)]
    pub fn receive(
        output: &SendOutput,
        recipient_sk: &KemSecretKey,
        sender: &NodeId,
    ) -> Result<Self> {
        Self::receive_chunks(std::slice::from_ref(output), recipient_sk, sender)
    }

    /// Decrypt chunks, restore their sequence order, and save only when complete.
    /// The input may arrive in any order; duplicates, gaps, and inconsistent
    /// file headers are rejected rather than producing a partial/corrupt file.
    #[allow(dead_code)]
    pub fn receive_chunks(
        outputs: &[SendOutput],
        recipient_sk: &KemSecretKey,
        sender: &NodeId,
    ) -> Result<Self> {
        let (filename, data) = Self::assemble_chunks(outputs, recipient_sk)?;
        Self::save(sender, &filename, &data)
    }

    fn assemble_chunks(
        outputs: &[SendOutput],
        recipient_sk: &KemSecretKey,
    ) -> Result<(String, Vec<u8>)> {
        if outputs.is_empty() {
            anyhow::bail!("aucun bloc Drive reçu");
        }
        if outputs.len() > MAX_TRANSFER_BYTES.div_ceil(CHUNK_SIZE) {
            anyhow::bail!("trop de blocs Drive pour la limite de transfert");
        }

        let mut decoded = Vec::new();
        let mut decoded_size = 0usize;

        for output in outputs {
            let plaintext = crate::msg::receive_bytes(output, recipient_sk)?;
            let chunk = decode_chunk(&plaintext)?;
            decoded_size = checked_transfer_size(decoded_size, chunk.data.len())?;
            decoded.push(chunk);
        }

        assemble_decoded_chunks(decoded)
    }
}

/// Read a file only up to `max_bytes`, including a one-byte probe to detect
/// growth after the metadata check without ever retaining an unbounded input.
pub(crate) fn read_file_capped(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > max_bytes as u64 {
        anyhow::bail!("fichier trop volumineux (limite {} octets)", max_bytes);
    }
    read_bytes_capped(file, max_bytes)
}

/// Read UTF-8 from stdin with the same hard byte cap used for file inputs.
pub(crate) fn read_stdin_capped(max_bytes: usize) -> Result<String> {
    let stdin = std::io::stdin();
    Ok(String::from_utf8(read_bytes_capped(
        stdin.lock(),
        max_bytes,
    )?)?)
}

fn read_bytes_capped(reader: impl Read, max_bytes: usize) -> Result<Vec<u8>> {
    let read_limit = u64::try_from(max_bytes)
        .ok()
        .and_then(|limit| limit.checked_add(1))
        .ok_or_else(|| anyhow::anyhow!("limite de lecture fichier invalide"))?;
    let mut data = Vec::new();
    reader.take(read_limit).read_to_end(&mut data)?;
    if data.len() > max_bytes {
        anyhow::bail!("entrée trop volumineuse (limite {} octets)", max_bytes);
    }
    Ok(data)
}

fn check_transfer_size(size: usize) -> Result<()> {
    if size > MAX_TRANSFER_BYTES {
        anyhow::bail!(
            "fichier trop volumineux pour le transfert (limite {} Mio)",
            MAX_TRANSFER_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

fn checked_transfer_size(current: usize, additional: usize) -> Result<usize> {
    let total = current
        .checked_add(additional)
        .ok_or_else(|| anyhow::anyhow!("taille totale du fichier hors limite mémoire"))?;
    check_transfer_size(total)?;
    Ok(total)
}

pub(crate) fn assemble_decoded_chunks(
    decoded: impl IntoIterator<Item = DecodedChunk>,
) -> Result<(String, Vec<u8>)> {
    let mut expected_total = None;
    let mut expected_filename: Option<String> = None;
    let mut expected_transfer_id = None;
    let mut chunks = BTreeMap::new();
    let mut total_size = 0usize;

    for chunk in decoded {
        if expected_total.is_some_and(|total| total != chunk.total) {
            anyhow::bail!("nombre de blocs incohérent dans le transfert Drive");
        }
        if expected_filename
            .as_ref()
            .is_some_and(|filename| filename != &chunk.filename)
        {
            anyhow::bail!("noms de fichier incohérents dans le transfert Drive");
        }
        if expected_transfer_id.is_some_and(|id| id != chunk.transfer_id) {
            anyhow::bail!("identifiant incohérent dans le transfert Drive");
        }
        expected_total = Some(chunk.total);
        expected_filename = Some(chunk.filename.clone());
        expected_transfer_id = Some(chunk.transfer_id);
        if chunks.contains_key(&chunk.seq) {
            anyhow::bail!("bloc Drive dupliqué");
        }
        total_size = checked_transfer_size(total_size, chunk.data.len())?;
        chunks.insert(chunk.seq, chunk.data);
    }

    let total = expected_total.expect("non-empty output list was checked");
    if chunks.len() != total {
        anyhow::bail!(
            "transfert Drive incomplet : {} bloc(s) reçu(s) sur {total}",
            chunks.len()
        );
    }

    let mut data = Vec::new();
    data.try_reserve(total_size)
        .map_err(|e| anyhow::anyhow!("allocation du fichier réassemblé impossible : {e}"))?;
    for (_, chunk) in chunks {
        data.extend_from_slice(&chunk);
    }
    Ok((
        expected_filename.expect("decoded filename was recorded"),
        data,
    ))
}

fn decode_chunk(payload: &[u8]) -> Result<DecodedChunk> {
    if payload.len() < CHUNK_HEADER_SIZE {
        anyhow::bail!("bloc Drive trop petit pour son en-tête");
    }
    if !payload.starts_with(CHUNK_MAGIC) {
        anyhow::bail!("signature de bloc Drive invalide");
    }
    let transfer_id_start = CHUNK_MAGIC.len();
    let seq_start = transfer_id_start + TRANSFER_ID_SIZE;
    let seq = u32::from_le_bytes(payload[seq_start..seq_start + 4].try_into()?) as usize;
    let total = u32::from_le_bytes(payload[seq_start + 4..seq_start + 8].try_into()?) as usize;
    let filename_len =
        u32::from_le_bytes(payload[seq_start + 8..seq_start + 12].try_into()?) as usize;
    if total == 0 || seq >= total {
        anyhow::bail!("séquence de bloc Drive invalide ({seq}/{total})");
    }
    let header_end = CHUNK_HEADER_SIZE
        .checked_add(filename_len)
        .ok_or_else(|| anyhow::anyhow!("longueur d'en-tête Drive hors limite"))?;
    if payload.len() < header_end {
        anyhow::bail!("bloc Drive tronqué : nom de fichier incomplet");
    }
    let filename = std::str::from_utf8(&payload[CHUNK_HEADER_SIZE..header_end])?;
    let filename = safe_filename(filename)?;
    let data = &payload[header_end..];
    let is_final = seq + 1 == total;
    if data.len() > CHUNK_SIZE
        || (!is_final && data.len() != CHUNK_SIZE)
        || (is_final && total > 1 && data.is_empty())
    {
        anyhow::bail!("taille de bloc Drive invalide à la séquence {seq}");
    }
    let mut transfer_id = [0u8; TRANSFER_ID_SIZE];
    transfer_id.copy_from_slice(&payload[transfer_id_start..seq_start]);
    Ok(DecodedChunk {
        transfer_id,
        seq,
        total,
        filename,
        data: data.to_vec(),
    })
}

pub(crate) fn safe_filename(filename: &str) -> Result<String> {
    let name = filename.rsplit(['/', '\\']).next().unwrap_or_default();
    if name.is_empty() || name == "." || name == ".." || name.contains('\0') {
        anyhow::bail!("nom de fichier Drive invalide");
    }
    if name.chars().any(char::is_control) {
        anyhow::bail!("nom de fichier Drive avec caractère de contrôle");
    }
    Ok(name.to_string())
}

/// Save without replacing an existing file; collisions receive a numbered name.
/// Uses a same-directory temporary and an atomic hard-link publication. Filesystems
/// without hard-link support fail closed; this never falls back to a replacing rename.
pub(crate) fn save_received_in(
    dir: &Path,
    filename: &str,
    data: &[u8],
) -> Result<(String, PathBuf)> {
    save_received_in_with(dir, filename, |file| file.write_all(data))
}

struct StagedReceivedFile {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl StagedReceivedFile {
    fn create(dir: &Path) -> Result<Self> {
        for _ in 0..10_000 {
            let id = NEXT_RECEIVED_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = dir.join(format!(
                "{RECEIVED_TEMP_PREFIX}{}-{id}.tmp",
                std::process::id()
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file: Some(file),
                    })
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        anyhow::bail!("impossible de créer un fichier temporaire Drive unique")
    }
}

impl Drop for StagedReceivedFile {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

fn save_received_in_with(
    dir: &Path,
    filename: &str,
    write_data: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(String, PathBuf)> {
    let name = safe_filename(filename)?;
    std::fs::create_dir_all(dir)?;
    let mut staged = StagedReceivedFile::create(dir)?;
    write_data(staged.file.as_mut().expect("staged file remains open"))?;
    staged
        .file
        .as_ref()
        .expect("staged file remains open")
        .sync_all()?;

    let source = Path::new(&name);
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or(&name);
    let extension = source.extension().and_then(|s| s.to_str());

    for suffix in 0..=10_000 {
        let candidate = if suffix == 0 {
            name.clone()
        } else if let Some(extension) = extension {
            format!("{stem} ({suffix}).{extension}")
        } else {
            format!("{name} ({suffix})")
        };
        let path = dir.join(&candidate);
        match std::fs::hard_link(&staged.path, &path) {
            Ok(()) => return Ok((candidate, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    anyhow::bail!("trop de fichiers reçus portent le même nom")
}

#[cfg(test)]
mod tests {
    use super::*;
    use polygone_core::crypto::kem;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct InjectedReader {
        data: Vec<u8>,
        position: usize,
        fail_at: Option<usize>,
        modified_after: Option<usize>,
        snapshot: FileSnapshot,
    }

    impl Read for InjectedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.fail_at == Some(self.position) {
                return Err(io::Error::other("erreur de lecture injectée"));
            }
            if self.position == self.data.len() || buffer.is_empty() {
                return Ok(0);
            }

            let mut count = (self.data.len() - self.position).min(buffer.len());
            if let Some(fail_at) = self.fail_at {
                count = count.min(fail_at.saturating_sub(self.position));
                if count == 0 {
                    return Err(io::Error::other("erreur de lecture injectée"));
                }
            }
            buffer[..count].copy_from_slice(&self.data[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    impl SendFileReader for InjectedReader {
        fn snapshot(&self) -> io::Result<FileSnapshot> {
            let mut snapshot = self.snapshot.clone();
            if self
                .modified_after
                .is_some_and(|position| self.position >= position)
            {
                snapshot.modified += std::time::Duration::from_secs(1);
            }
            Ok(snapshot)
        }
    }

    fn injected_file_iterator<'a>(
        recipient: &'a KemPublicKey,
        initial_size: usize,
        actual_data: Vec<u8>,
        fail_at: Option<usize>,
    ) -> SendFileChunks<'a, InjectedReader> {
        let snapshot = FileSnapshot {
            len: initial_size as u64,
            modified: SystemTime::now(),
        };
        SendFileChunks {
            file: InjectedReader {
                data: actual_data,
                position: 0,
                fail_at,
                modified_after: None,
                snapshot: snapshot.clone(),
            },
            filename: "injected.bin".to_string(),
            recipient,
            transfer_id: [0x55; TRANSFER_ID_SIZE],
            snapshot,
            total_chunks: initial_size.max(1).div_ceil(CHUNK_SIZE) as u32,
            next_seq: 0,
        }
    }

    #[test]
    fn multi_chunk_file_reassembles_when_chunks_arrive_out_of_order() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let sender = NodeId([7; 16]);
        let data: Vec<u8> = (0..(CHUNK_SIZE * 2 + 317))
            .map(|i| (i % 251) as u8)
            .collect();
        let mut outputs = DriveFile::prepare_send(&data, "archive.bin", &recipient_pk).unwrap();
        assert_eq!(outputs.len(), 3);
        outputs.reverse();

        let (filename, rebuilt) = DriveFile::assemble_chunks(&outputs, &recipient_sk).unwrap();
        assert_eq!(filename, "archive.bin");
        assert_eq!(rebuilt, data);

        let dir = unique_temp_dir();
        let received = DriveFile::save_in(&dir, &sender, &filename, &rebuilt).unwrap();
        assert_eq!(received.sender, sender);
        assert_eq!(std::fs::read(&received.path).unwrap(), data);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_file_uses_one_decryptable_chunk() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let output = DriveFile::prepare_send(&[], "empty.dat", &recipient_pk).unwrap();
        assert_eq!(output.len(), 1);
        let (filename, bytes) = DriveFile::assemble_chunks(&output, &recipient_sk).unwrap();
        assert_eq!(filename, "empty.dat");
        assert!(bytes.is_empty());
    }

    #[test]
    fn missing_and_duplicate_chunks_are_rejected() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let data = vec![42; CHUNK_SIZE + 1];
        let outputs = DriveFile::prepare_send(&data, "two.bin", &recipient_pk).unwrap();
        assert!(DriveFile::assemble_chunks(&outputs[..1], &recipient_sk).is_err());

        let duplicate = vec![outputs[0].clone(), outputs[0].clone()];
        let err = DriveFile::assemble_chunks(&duplicate, &recipient_sk).unwrap_err();
        assert!(err.to_string().contains("dupliqué"));
    }

    #[test]
    fn drive_filename_cannot_escape_received_directory() {
        assert_eq!(safe_filename("../../secret.txt").unwrap(), "secret.txt");
        assert_eq!(safe_filename(r"..\secret.txt").unwrap(), "secret.txt");
        assert!(safe_filename("../").is_err());
    }

    #[test]
    fn drive_filename_rejects_terminal_control_characters() {
        assert!(safe_filename("line\nbreak.txt").is_err());
        assert!(safe_filename("\u{1b}[2J.txt").is_err());
    }

    #[test]
    fn saving_a_received_file_never_overwrites_an_existing_file() {
        let dir = unique_temp_dir();
        let sender = NodeId([8; 16]);
        let first = DriveFile::save_in(&dir, &sender, "report.txt", b"first").unwrap();
        let second = DriveFile::save_in(&dir, &sender, "report.txt", b"second").unwrap();

        assert_eq!(first.name, "report.txt");
        assert_eq!(second.name, "report (1).txt");
        assert_eq!(std::fs::read(first.path).unwrap(), b"first");
        assert_eq!(std::fs::read(second.path).unwrap(), b"second");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn interrupted_write_does_not_publish_a_partial_received_file() {
        let dir = unique_temp_dir();
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = save_received_in_with(&dir, "interrupted.bin", |file| {
                file.write_all(b"first half")?;
                panic!("simulated interruption during received-file write");
            });
        }));

        assert!(interrupted.is_err());
        let leftover_entries = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(
            leftover_entries, 0,
            "an interrupted write left a partial file"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn injected_write_failure_does_not_leave_a_partial_received_file() {
        let dir = unique_temp_dir();
        let result = save_received_in_with(&dir, "failed.bin", |file| {
            file.write_all(b"first half")?;
            Err(std::io::Error::other("injected write failure"))
        });

        assert!(result.is_err());
        assert!(!dir.join("failed.bin").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_empty_final_chunk_in_a_multi_chunk_file() {
        let mut payload = Vec::new();
        payload.extend_from_slice(CHUNK_MAGIC);
        payload.extend_from_slice(&[0; TRANSFER_ID_SIZE]);
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(&2u32.to_le_bytes());
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(b"x");

        assert!(decode_chunk(&payload).is_err());
    }

    #[test]
    fn chunks_share_transfer_id_and_separate_transfers_do_not() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let data = vec![7; CHUNK_SIZE + 1];
        let first =
            DriveFile::prepare_send_with_id(&data, "file.bin", &recipient_pk, [1; 16]).unwrap();
        let second =
            DriveFile::prepare_send_with_id(&data, "file.bin", &recipient_pk, [2; 16]).unwrap();
        let first_ids: Vec<_> = first
            .iter()
            .map(|output| {
                let plain = crate::msg::receive_bytes(output, &recipient_sk).unwrap();
                decode_chunk(&plain).unwrap().transfer_id
            })
            .collect();
        let second_id = crate::msg::receive_bytes(&second[0], &recipient_sk).unwrap();
        assert!(first_ids.iter().all(|id| *id == [1; 16]));
        assert_eq!(decode_chunk(&second_id).unwrap().transfer_id, [2; 16]);
    }

    #[test]
    fn lazy_chunks_match_batch_blocks_ids_order_and_wire_format() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let transfer_id = [0x5a; TRANSFER_ID_SIZE];
        let data: Vec<_> = (0..(CHUNK_SIZE * 2 + 317))
            .map(|index| (index % 251) as u8)
            .collect();
        let batch =
            DriveFile::prepare_send_with_id(&data, "equivalence.bin", &recipient_pk, transfer_id)
                .unwrap();
        let lazy: Vec<_> = DriveFile::prepare_send_iter_with_id(
            &data,
            "equivalence.bin",
            &recipient_pk,
            transfer_id,
        )
        .unwrap()
        .collect::<Result<_>>()
        .unwrap();

        assert_eq!(lazy.len(), batch.len());
        for (seq, (batch_output, lazy_output)) in batch.iter().zip(&lazy).enumerate() {
            let batch_plain = crate::msg::receive_bytes(batch_output, &recipient_sk).unwrap();
            let lazy_plain = crate::msg::receive_bytes(lazy_output, &recipient_sk).unwrap();
            assert_eq!(lazy_plain, batch_plain, "bloc {seq} diffère du batch");

            let decoded = decode_chunk(&lazy_plain).unwrap();
            assert_eq!(decoded.transfer_id, transfer_id);
            assert_eq!(decoded.seq, seq);
            assert_eq!(decoded.total, batch.len());
            assert_eq!(decoded.filename, "equivalence.bin");
            let start = seq * CHUNK_SIZE;
            let end = start.saturating_add(CHUNK_SIZE).min(data.len());
            assert_eq!(decoded.data, data[start..end]);

            let serialized = lazy_output.display();
            let parsed = SendOutput::parse(&serialized).unwrap();
            assert_eq!(
                crate::msg::receive_bytes(&parsed, &recipient_sk).unwrap(),
                lazy_plain,
                "format wire non compatible pour le bloc {seq}"
            );
        }
    }

    #[test]
    fn lazy_consumer_processes_one_output_at_a_time_in_stable_order() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let data = vec![0x7b; CHUNK_SIZE * 3 + 19];
        let iterator = DriveFile::prepare_send_iter(&data, "streamed.bin", &recipient_pk).unwrap();
        assert_eq!(iterator.len(), 4);

        let mut consumed = 0;
        let mut transfer_id = None;
        for (expected_seq, output) in iterator.enumerate() {
            let output = output.unwrap();
            let plaintext = crate::msg::receive_bytes(&output, &recipient_sk).unwrap();
            let decoded = decode_chunk(&plaintext).unwrap();
            assert_eq!(decoded.seq, expected_seq);
            if let Some(expected_id) = transfer_id {
                assert_eq!(decoded.transfer_id, expected_id);
            } else {
                transfer_id = Some(decoded.transfer_id);
            }
            assert_eq!(decoded.filename, "streamed.bin");
            consumed += 1;
            // `output` is scoped to this iteration and is dropped before the
            // next one; no collection of SendOutput is retained by this consumer.
        }
        assert_eq!(consumed, 4);
    }

    #[test]
    fn send_iter_reports_early_eof_without_a_file_race() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let initial_size = CHUNK_SIZE * 2;
        let mut iterator = injected_file_iterator(
            &recipient_pk,
            initial_size,
            vec![0x31; initial_size - 1],
            None,
        );

        assert!(iterator.next().unwrap().is_ok());
        let error = iterator.next().unwrap().unwrap_err();
        assert!(error.to_string().contains("tronqué"));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn send_iter_rejects_an_extra_byte_without_a_file_race() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let initial_size = CHUNK_SIZE;
        let mut actual_data = vec![0x32; initial_size];
        actual_data.push(0x33);
        let mut iterator = injected_file_iterator(&recipient_pk, initial_size, actual_data, None);

        let error = iterator.next().unwrap().unwrap_err();
        assert!(error.to_string().contains("agrandi"));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn send_iter_rechecks_metadata_visible_same_size_modification() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let initial_size = CHUNK_SIZE * 2;
        let mut iterator =
            injected_file_iterator(&recipient_pk, initial_size, vec![0x35; initial_size], None);
        iterator.file.modified_after = Some(initial_size);

        assert!(iterator.next().unwrap().is_ok());
        let error = iterator.next().unwrap().unwrap_err();
        assert!(error.to_string().contains("modifié"));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn send_iter_stops_after_a_mid_stream_read_error() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let initial_size = CHUNK_SIZE * 2;
        let mut iterator = injected_file_iterator(
            &recipient_pk,
            initial_size,
            vec![0x34; initial_size],
            Some(CHUNK_SIZE),
        );

        assert!(iterator.next().unwrap().is_ok());
        let error = iterator.next().unwrap().unwrap_err();
        assert!(format!("{error:#}").contains("erreur de lecture injectée"));
        assert!(iterator.next().is_none());
    }

    #[test]
    fn lazy_iterator_stops_after_the_first_chunk_preparation_error() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let data = vec![4; CHUNK_SIZE * 4];
        let calls = std::cell::RefCell::new(Vec::new());
        let mut iterator = DriveFile::prepare_send_iter_with_preparer(
            &data,
            "failure.bin",
            &recipient_pk,
            [0x44; TRANSFER_ID_SIZE],
            Box::new(|chunk, filename, recipient, transfer_id, seq, total| {
                calls.borrow_mut().push(seq);
                if seq == 2 {
                    anyhow::bail!("erreur injectée au bloc {seq}");
                }
                DriveFile::prepare_send_chunk(chunk, filename, recipient, transfer_id, seq, total)
            }),
        )
        .unwrap();

        assert!(iterator.next().unwrap().is_ok());
        assert!(iterator.next().unwrap().is_ok());
        assert!(iterator.next().unwrap().is_err());
        assert!(iterator.next().is_none());
        drop(iterator);
        assert_eq!(*calls.borrow(), [0, 1, 2]);
    }

    #[test]
    fn lazy_iterator_validates_the_transfer_limit_before_emitting() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let oversized = vec![0; MAX_TRANSFER_BYTES + 1];
        let calls = std::cell::Cell::new(0);
        let result = DriveFile::prepare_send_iter_with_preparer(
            &oversized,
            "too-large.bin",
            &recipient_pk,
            [0; TRANSFER_ID_SIZE],
            Box::new(|_, _, _, _, _, _| {
                calls.set(calls.get() + 1);
                anyhow::bail!("ne doit pas être appelé")
            }),
        );

        assert!(result.err().unwrap().to_string().contains("limite 256 Mio"));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn file_send_iter_matches_send_and_uses_stable_block_order() {
        let (recipient_pk, recipient_sk) = kem::generate_keypair().unwrap();
        let data: Vec<_> = (0..(CHUNK_SIZE * 2 + 23))
            .map(|index| (index % 239) as u8)
            .collect();
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("streamed-file.bin");
        std::fs::write(&path, &data).unwrap();

        let batch = DriveFile::send(&recipient_pk, &path).unwrap();
        let iterator = DriveFile::send_iter(&recipient_pk, &path).unwrap();
        assert_eq!(iterator.len(), batch.len());
        for (expected_seq, output) in iterator.enumerate() {
            let decoded =
                decode_chunk(&crate::msg::receive_bytes(&output.unwrap(), &recipient_sk).unwrap())
                    .unwrap();
            assert_eq!(decoded.seq, expected_seq);
            assert_eq!(decoded.total, batch.len());
            assert_eq!(decoded.filename, "streamed-file.bin");
            let start = expected_seq * CHUNK_SIZE;
            let end = start.saturating_add(CHUNK_SIZE).min(data.len());
            assert_eq!(decoded.data, data[start..end]);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_send_iter_checks_the_256_mib_limit_before_reading() {
        let (recipient_pk, _) = kem::generate_keypair().unwrap();
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("limit.bin");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_TRANSFER_BYTES as u64).unwrap();
        let iterator = DriveFile::send_iter(&recipient_pk, &path).unwrap();
        assert_eq!(iterator.len(), MAX_TRANSFER_BYTES.div_ceil(CHUNK_SIZE));
        drop(iterator);

        file.set_len(MAX_TRANSFER_BYTES as u64 + 1).unwrap();
        let error = DriveFile::send_iter(&recipient_pk, &path).err().unwrap();
        assert!(error.to_string().contains("limite 256 Mio"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn capped_file_read_accepts_limit_and_rejects_oversize() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload.bin");
        std::fs::write(&path, b"12345678").unwrap();
        assert_eq!(read_file_capped(&path, 8).unwrap(), b"12345678");
        std::fs::write(&path, b"123456789").unwrap();
        let error = read_file_capped(&path, 8).unwrap_err();
        assert!(error.to_string().contains("trop volumineux"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn capped_reader_accepts_limit_and_rejects_oversize() {
        let exact = std::io::Cursor::new(b"12345678".to_vec());
        assert_eq!(read_bytes_capped(exact, 8).unwrap(), b"12345678");

        let oversized = std::io::Cursor::new(b"123456789".to_vec());
        let error = read_bytes_capped(oversized, 8).unwrap_err();
        assert!(error.to_string().contains("trop volumineuse"));
    }

    #[test]
    fn batch_transfer_size_limit_is_inclusive() {
        assert!(check_transfer_size(MAX_TRANSFER_BYTES - 1).is_ok());
        assert!(check_transfer_size(MAX_TRANSFER_BYTES).is_ok());

        let error = check_transfer_size(MAX_TRANSFER_BYTES + 1).unwrap_err();
        assert!(error.to_string().contains("limite 256 Mio"));
    }

    #[test]
    fn decoded_reassembly_enforces_cumulative_transfer_size_limit() {
        for size in [MAX_TRANSFER_BYTES - 1, MAX_TRANSFER_BYTES] {
            let (filename, data) = assemble_decoded_chunks(decoded_chunks_of_size(size)).unwrap();
            assert_eq!(filename, "boundary.bin");
            assert_eq!(data.len(), size);
        }

        let error =
            assemble_decoded_chunks(decoded_chunks_of_size(MAX_TRANSFER_BYTES + 1)).unwrap_err();
        assert!(error.to_string().contains("limite 256 Mio"));
    }

    fn decoded_chunks_of_size(size: usize) -> Vec<DecodedChunk> {
        let total = size.max(1).div_ceil(CHUNK_SIZE);
        (0..total)
            .map(|seq| {
                let start = seq * CHUNK_SIZE;
                let chunk_size = size.saturating_sub(start).min(CHUNK_SIZE);
                DecodedChunk {
                    transfer_id: [9; TRANSFER_ID_SIZE],
                    seq,
                    total,
                    filename: "boundary.bin".to_string(),
                    data: vec![0; chunk_size],
                }
            })
            .collect()
    }

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("polygone-drive-{}-{nanos}", std::process::id()))
    }
}
