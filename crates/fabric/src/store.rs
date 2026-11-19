//! Magasin adressé par contenu et plan de réplication.
//!
//! **Ce module n'est pas Babel.** La recherche historique n'a retrouvé aucune
//! spécification de Babel dans le code : seulement des références à la
//! « bibliothèque de Babel » et l'usage d'un adressage par hash dans `calc.rs`.
//! `ADR-014` (`Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE`) garde
//! la décision ouverte. Ce module fournit donc uniquement ce qui est
//! vérifiable : découpage en blocs BLAKE3, manifeste, plan de réplication,
//! détection de corruption et reconstruction depuis les répliques.
//!
//! Il ne revendique **ni compression, ni stockage infini, ni recherche
//! exhaustive**. Un bloc est adressé par son contenu ; le hash prouve
//! l'intégrité des octets, pas leur vérité ni leur disponibilité.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Taille maximale d'un bloc : 1 Mio, comme le magasin de blocs existant.
pub const MAX_CHUNK_BYTES: usize = 1024 * 1024;
/// Longueur d'un hash BLAKE3-256 en hexadécimal.
pub const HASH_HEX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    #[error("identifiant de bloc invalide : {0}")]
    InvalidBlockId(String),
    #[error("données vides : aucun bloc à adresser")]
    EmptyData,
    #[error("taille de bloc invalide : {0} (1..={MAX_CHUNK_BYTES})")]
    InvalidChunkSize(usize),
    #[error("bloc {id} trop gros : {size} octets")]
    ChunkTooLarge { id: String, size: usize },
    #[error("bloc {id} altéré : hash recalculé différent")]
    ChunkHashMismatch { id: String },
    #[error("bloc {id} de longueur {actual}, attendu {expected}")]
    ChunkLengthMismatch {
        id: String,
        expected: u64,
        actual: u64,
    },
    #[error("bloc {0} absent de la reconstruction")]
    MissingChunk(String),
    #[error("racine du manifeste incohérente")]
    ManifestRootMismatch,
    #[error("total du manifeste incohérent : {actual} octets, attendu {expected}")]
    TotalMismatch { expected: u64, actual: u64 },
    #[error("réplication invalide : {0}")]
    InvalidReplication(usize),
    #[error("aucun nœud de stockage")]
    NoNodes,
    #[error("capacité insuffisante pour placer toutes les répliques")]
    InsufficientCapacity,
}

/// Identifiant de bloc : BLAKE3-256 en hexadécimal minuscule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BlockId(String);

impl BlockId {
    pub fn parse(value: impl Into<String>) -> Result<Self, StoreError> {
        let value = value.into();
        if value.len() != HASH_HEX_LEN
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StoreError::InvalidBlockId(value));
        }
        Ok(Self(value))
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for BlockId {
    type Error = StoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<BlockId> for String {
    fn from(value: BlockId) -> Self {
        value.0
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Un bloc décrit par son identifiant et sa longueur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub id: BlockId,
    pub len: u64,
}

/// Blocs produits par un découpage : identifiant et octets.
pub type ChunkedContent = Vec<(BlockId, Vec<u8>)>;

/// Manifeste complet d'un contenu découpé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentManifest {
    pub total_bytes: u64,
    pub chunks: Vec<Chunk>,
    /// BLAKE3 de la liste ordonnée des identifiants et longueurs.
    pub root: String,
}

fn root_of(chunks: &[Chunk]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"polygone-content-manifest-v1");
    for chunk in chunks {
        hasher.update(chunk.id.as_str().as_bytes());
        hasher.update(&chunk.len.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

/// Découpe `data` en blocs adressés par contenu.
///
/// Rend le manifeste et les octets de chaque bloc. Aucun octet n'est perdu :
/// la concaténation des blocs redonne exactement `data`.
pub fn chunk_data(
    data: &[u8],
    max_chunk: usize,
) -> Result<(ContentManifest, ChunkedContent), StoreError> {
    if data.is_empty() {
        return Err(StoreError::EmptyData);
    }
    if max_chunk == 0 || max_chunk > MAX_CHUNK_BYTES {
        return Err(StoreError::InvalidChunkSize(max_chunk));
    }
    let mut chunks = Vec::new();
    let mut payloads = Vec::new();
    for part in data.chunks(max_chunk) {
        if part.len() > MAX_CHUNK_BYTES {
            return Err(StoreError::ChunkTooLarge {
                id: "nouveau".to_string(),
                size: part.len(),
            });
        }
        let id = BlockId::from_bytes(part);
        chunks.push(Chunk {
            id: id.clone(),
            len: part.len() as u64,
        });
        payloads.push((id, part.to_vec()));
    }
    let manifest = ContentManifest {
        total_bytes: data.len() as u64,
        root: root_of(&chunks),
        chunks,
    };
    Ok((manifest, payloads))
}

/// Vérifie un bloc : longueur annoncée, hash recalculé.
pub fn verify_chunk(chunk: &Chunk, data: &[u8]) -> Result<(), StoreError> {
    if data.len() as u64 != chunk.len {
        return Err(StoreError::ChunkLengthMismatch {
            id: chunk.id.to_string(),
            expected: chunk.len,
            actual: data.len() as u64,
        });
    }
    if BlockId::from_bytes(data) != chunk.id {
        return Err(StoreError::ChunkHashMismatch {
            id: chunk.id.to_string(),
        });
    }
    Ok(())
}

/// Reconstruit le contenu à partir des blocs disponibles, en vérifiant chaque
/// bloc puis la racine du manifeste. Un bloc corrompu est refusé, jamais
/// concaténé « au cas où ».
pub fn reconstruct(
    manifest: &ContentManifest,
    available: &HashMap<BlockId, Vec<u8>>,
) -> Result<Vec<u8>, StoreError> {
    let mut output = Vec::with_capacity(manifest.total_bytes as usize);
    for chunk in &manifest.chunks {
        let data = available
            .get(&chunk.id)
            .ok_or_else(|| StoreError::MissingChunk(chunk.id.to_string()))?;
        verify_chunk(chunk, data)?;
        output.extend_from_slice(data);
    }
    if output.len() as u64 != manifest.total_bytes {
        return Err(StoreError::TotalMismatch {
            expected: manifest.total_bytes,
            actual: output.len() as u64,
        });
    }
    if root_of(&manifest.chunks) != manifest.root {
        return Err(StoreError::ManifestRootMismatch);
    }
    Ok(output)
}

/// Placement d'un bloc sur `replication` nœuds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaAssignment {
    pub chunk: BlockId,
    pub nodes: Vec<String>,
}

/// Plan de réplication déterministe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaPlan {
    pub replication: usize,
    pub assignments: Vec<ReplicaAssignment>,
}

impl ReplicaPlan {
    /// Nœuds qui détiennent ce bloc.
    pub fn nodes_for(&self, chunk: &BlockId) -> Option<&[String]> {
        self.assignments
            .iter()
            .find(|assignment| &assignment.chunk == chunk)
            .map(|assignment| assignment.nodes.as_slice())
    }

    /// Blocs sans aucune réplique parmi les nœuds disponibles.
    pub fn missing(&self, available: &[String]) -> Vec<BlockId> {
        self.assignments
            .iter()
            .filter(|assignment| {
                !assignment
                    .nodes
                    .iter()
                    .any(|node| available.iter().any(|up| up == node))
            })
            .map(|assignment| assignment.chunk.clone())
            .collect()
    }

    pub fn is_available(&self, available: &[String]) -> bool {
        self.missing(available).is_empty()
    }
}

/// Place chaque bloc sur `replication` nœuds, en tournant pour répartir la
/// charge. Un nœud dont la capacité restante ne suffit pas est ignoré ; si le
/// nombre de répliques ne peut pas être atteint, le plan est refusé.
pub fn replica_plan(
    manifest: &ContentManifest,
    nodes: &[(String, u64)],
    replication: usize,
) -> Result<ReplicaPlan, StoreError> {
    if nodes.is_empty() {
        return Err(StoreError::NoNodes);
    }
    if replication == 0 {
        return Err(StoreError::InvalidReplication(replication));
    }
    let replication = replication.min(nodes.len());
    let mut remaining: Vec<(String, u64)> = nodes.to_vec();
    let mut assignments = Vec::with_capacity(manifest.chunks.len());

    for (index, chunk) in manifest.chunks.iter().enumerate() {
        let start = index % remaining.len();
        let mut placed: Vec<String> = Vec::with_capacity(replication);
        for offset in 0..remaining.len() {
            if placed.len() == replication {
                break;
            }
            let slot = (start + offset) % remaining.len();
            if remaining[slot].1 >= chunk.len {
                remaining[slot].1 -= chunk.len;
                placed.push(remaining[slot].0.clone());
            }
        }
        if placed.len() != replication {
            return Err(StoreError::InsufficientCapacity);
        }
        assignments.push(ReplicaAssignment {
            chunk: chunk.id.clone(),
            nodes: placed,
        });
    }

    Ok(ReplicaPlan {
        replication,
        assignments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_is_lossless_and_content_addressed() {
        let data = vec![7u8; 10_000];
        let (manifest, payloads) = chunk_data(&data, 4096).expect("découpage");
        assert_eq!(payloads.len(), 3);
        assert_eq!(manifest.total_bytes, 10_000);
        let map: HashMap<BlockId, Vec<u8>> = payloads.into_iter().collect();
        let rebuilt = reconstruct(&manifest, &map).expect("reconstruction");
        assert_eq!(rebuilt, data);
        assert_eq!(manifest.chunks[0].id, BlockId::from_bytes(&data[..4096]));
    }

    #[test]
    fn a_corrupted_block_is_rejected() {
        let data = b"contenu authentique".to_vec();
        let (manifest, payloads) = chunk_data(&data, 4096).expect("découpage");
        let mut map: HashMap<BlockId, Vec<u8>> = payloads.into_iter().collect();
        let id = manifest.chunks[0].id.clone();
        map.get_mut(&id).expect("bloc").push(0);
        let error = reconstruct(&manifest, &map).expect_err("corruption refusée");
        assert!(
            matches!(error, StoreError::ChunkLengthMismatch { .. }),
            "{error:?}"
        );

        let mut map: HashMap<BlockId, Vec<u8>> = HashMap::new();
        let mut tampered = data.clone();
        tampered[0] ^= 0x01;
        map.insert(id, tampered);
        let error = reconstruct(&manifest, &map).expect_err("hash refusé");
        assert!(
            matches!(error, StoreError::ChunkHashMismatch { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_missing_block_is_named_not_silently_ignored() {
        let data = vec![1u8; 9000];
        let (manifest, payloads) = chunk_data(&data, 4096).expect("découpage");
        let mut map: HashMap<BlockId, Vec<u8>> = payloads.into_iter().collect();
        let missing = manifest.chunks[1].id.clone();
        map.remove(&missing);
        let error = reconstruct(&manifest, &map).expect_err("bloc absent");
        match error {
            StoreError::MissingChunk(id) => assert_eq!(id, missing.to_string()),
            other => panic!("erreur inattendue : {other:?}"),
        }
    }

    #[test]
    fn replication_survives_one_node_loss_and_reports_two() {
        let data = vec![3u8; 12_000];
        let (manifest, _) = chunk_data(&data, 4096).expect("découpage");
        let nodes = vec![
            ("node-a".to_string(), 1_000_000u64),
            ("node-b".to_string(), 1_000_000u64),
            ("node-c".to_string(), 1_000_000u64),
        ];
        let plan = replica_plan(&manifest, &nodes, 2).expect("plan");
        assert_eq!(plan.replication, 2);
        assert!(plan.is_available(&["node-a".into(), "node-b".into(), "node-c".into()]));
        assert!(
            plan.is_available(&["node-b".into(), "node-c".into()]),
            "{plan:?}"
        );
        let missing = plan.missing(&["node-b".into()]);
        assert!(
            !missing.is_empty(),
            "une seule réplique ne suffit pas toujours"
        );
    }

    #[test]
    fn insufficient_capacity_is_refused() {
        let data = vec![0u8; 9000];
        let (manifest, _) = chunk_data(&data, 4096).expect("découpage");
        let nodes = vec![("small".to_string(), 1024u64)];
        let error = replica_plan(&manifest, &nodes, 1).expect_err("capacité");
        assert_eq!(error, StoreError::InsufficientCapacity);
    }

    #[test]
    fn block_ids_reject_traversal_and_uppercase() {
        for hostile in [
            "../../etc/passwd".to_string(),
            "A".repeat(64),
            "0".repeat(63),
            "g".repeat(64),
        ] {
            assert!(BlockId::parse(hostile).is_err());
        }
    }
}
