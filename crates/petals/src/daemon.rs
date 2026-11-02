//! Daemon integration for Petals — resource allocation via `polygoned`.
//!
//! Le client parle **le protocole du daemon** (`polygoned::socket`) : même
//! chemin de socket, même champ `v`, mêmes noms de types, mêmes champs.
//!
//! Aucune copie locale du format — c'était la source de la dérive. Ce client
//! envoyait `AllocateResources`, `GetSystemResources` et `RegisterModel`, que
//! le daemon ne connaît pas, sans le champ `v` que le protocole v1 exige sur
//! chaque message, vers `/tmp/polygoned.sock` alors que le daemon écoute
//! `~/.polygone/daemon.sock`. Aucun de ses messages n'était accepté.
//!
//! Ce qui reste propre à Petals, c'est l'**API d'appel** (`ResourceRequest`,
//! `ResourceAllocation`) : elle est *traduite* vers le protocole, pas dupliquée.

use crate::types::ModelRequirements;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use polygoned::socket::{decode_reply, encode_request, socket_path, Reply, Request};

/// Identifié que le daemon attribue à ce client dans ses journaux.
pub const CLIENT_NAME: &str = "petals";

/// Client for communicating with the Polygone daemon (`polygoned`).
pub struct PetalsDaemonClient {
    socket_path: PathBuf,
    reader: Option<BufReader<tokio::net::unix::OwnedReadHalf>>,
    writer: Option<tokio::net::unix::OwnedWriteHalf>,
    allocation_id: Option<String>,
}

impl PetalsDaemonClient {
    /// Connect to the daemon
    pub async fn connect() -> Result<Self> {
        // Le chemin est celui du daemon : même variable d'environnement,
        // même repli `~/.polygone/daemon.sock`. Deux règles, deux chemins,
        // c'est la dérive qu'on vient de corriger.
        Ok(Self {
            socket_path: socket_path(),
            reader: None,
            writer: None,
            allocation_id: None,
        })
    }

    /// Le chemin du socket réellement visé — pour l'affichage et les tests.
    pub fn path(&self) -> &PathBuf {
        &self.socket_path
    }

    /// Ensure connection is established
    async fn ensure_connected(&mut self) -> Result<()> {
        if self.reader.is_none() {
            let stream = UnixStream::connect(&self.socket_path).await?;
            let (reader, writer) = stream.into_split();
            self.reader = Some(BufReader::new(reader));
            self.writer = Some(writer);
        }
        Ok(())
    }

    /// Send a request to the daemon and read its reply.
    ///
    /// L'encodage et le décodage sont ceux du daemon : le champ `v` est posé
    /// par `encode_request`, en un seul endroit, comme le veut le protocole.
    async fn send_request(&mut self, request: &Request) -> Result<Reply> {
        self.ensure_connected().await?;

        let line = encode_request(request);
        let writer = self.writer.as_mut().unwrap();
        writer.write_all(line.as_bytes()).await?;
        writer.flush().await?;

        let reader = self.reader.as_mut().unwrap();
        let mut response = String::new();
        reader.read_line(&mut response).await?;
        decode_reply(response.trim_end())
            .map_err(|err| anyhow!("réponse du daemon illisible : {err:?}"))
    }

    /// Request resource allocation for Petals models
    pub async fn request_allocation(
        &mut self,
        requirements: ResourceRequest,
    ) -> Result<ResourceAllocation> {
        let request = alloc_request(&requirements);
        let reply = self.send_request(&request).await?;
        match &reply {
            Reply::AllocGranted { .. } => {
                let allocation = allocation_from_reply(&reply)
                    .ok_or_else(|| anyhow!("réponse sans allocation"))?;
                self.allocation_id = Some(allocation.id.clone());
                Ok(allocation)
            }
            Reply::AllocDenied { reason, .. } => {
                anyhow::bail!("allocation refusée par le daemon : {reason}")
            }
            Reply::Error { message, .. } => {
                anyhow::bail!("erreur du daemon : {message}")
            }
            other => anyhow::bail!("réponse inattendue du daemon : {other:?}"),
        }
    }

    /// Release the current allocation
    pub async fn release_allocation(&mut self) -> Result<()> {
        let Some(allocation_id) = self.allocation_id.take() else {
            return Ok(());
        };
        let request = release_request(&allocation_id);
        match self.send_request(&request).await? {
            Reply::Released { .. } => Ok(()),
            Reply::Error { message, .. } => anyhow::bail!("libération refusée : {message}"),
            other => anyhow::bail!("réponse inattendue du daemon : {other:?}"),
        }
    }

    /// L'allocation courante telle que le daemon la rapporte.
    ///
    /// Le daemon expose l'allocation, pas un état de la machine : ni la RAM
    /// totale, ni les cœurs libres. Les rapporter ici serait les inventer.
    pub async fn allocation_status(&mut self) -> Result<AllocationStatus> {
        let request = status_request();
        let reply = self.send_request(&request).await?;
        match &reply {
            Reply::Status { .. } => {
                status_from_reply(&reply).ok_or_else(|| anyhow!("réponse sans état d'allocation"))
            }
            Reply::Error { message, .. } => anyhow::bail!("erreur du daemon : {message}"),
            other => anyhow::bail!("réponse inattendue du daemon : {other:?}"),
        }
    }

    /// Enregistrer un modèle auprès du daemon.
    ///
    /// Le protocole v1 n'a **pas** de requête d'enregistrement de modèle :
    /// l'envoyer serait refusé (`UnknownType`). On le dit explicitement
    /// plutôt que d'envoyer un message que personne ne lira.
    pub async fn register_model(
        &mut self,
        _model: &str,
        _requirements: &ModelRequirements,
    ) -> Result<()> {
        anyhow::bail!(
            "le protocole du daemon n'expose pas d'enregistrement de modèle \
             (protocole v1 : status_request, ping, alloc_request, release)"
        )
    }
}

// ============================================================================
// Le fil — fonctions pures, testables sans socket
// ============================================================================

/// La requête d'allocation, dans le vocabulaire du daemon.
///
/// `concurrent_requests` et `model_names` n'ont pas de pendant dans le
/// protocole : le daemon tient **une** allocation globale, pas une allocation
/// par client. Ils ne sont donc pas transmis — et c'est dit ici, plutôt que
/// dans un TODO.
pub fn alloc_request(requirements: &ResourceRequest) -> Request {
    Request::Alloc {
        id: None,
        client: Some(CLIENT_NAME.to_string()),
        cpu_cores: requirements.cpu_cores,
        ram_gb: requirements.ram_gb,
        gpu_vram_gb: requirements.gpu_vram_gb,
        storage_gb: None,
    }
}

/// La requête de libération.
pub fn release_request(allocation_id: &str) -> Request {
    Request::Release {
        id: None,
        allocation_id: allocation_id.to_string(),
    }
}

/// La requête d'état.
pub fn status_request() -> Request {
    Request::Status { id: None }
}

/// Traduit une réponse d'allocation du daemon vers l'API de Petals.
///
/// Les chemins de cgroup ne sont pas fournis par le daemon : ils restent
/// `None` — un chemin inventé serait pire qu'un chemin absent.
pub fn allocation_from_reply(reply: &Reply) -> Option<ResourceAllocation> {
    match reply {
        Reply::AllocGranted {
            allocation_id,
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            expires_at,
            ..
        } => Some(ResourceAllocation {
            id: allocation_id.clone(),
            cpu_cores: *cpu_cores,
            ram_gb: *ram_gb,
            gpu_vram_gb: *gpu_vram_gb,
            cpu_cgroup: None,
            memory_cgroup: None,
            expires_at: expires_at.and_then(|secs| chrono::DateTime::from_timestamp(secs, 0)),
        }),
        _ => None,
    }
}

/// Traduit une réponse d'état du daemon vers l'API de Petals.
pub fn status_from_reply(reply: &Reply) -> Option<AllocationStatus> {
    match reply {
        Reply::Status {
            tier,
            cpu_cores,
            ram_bytes,
            gpu_free_vram_mb,
            max_cpu_percent,
            max_ram_percent,
            shrinking,
            uptime_secs,
            ..
        } => Some(AllocationStatus {
            tier: tier.clone(),
            cpu_cores: *cpu_cores,
            ram_bytes: *ram_bytes,
            gpu_free_vram_mb: *gpu_free_vram_mb,
            max_cpu_percent: *max_cpu_percent,
            max_ram_percent: *max_ram_percent,
            shrinking: *shrinking,
            uptime_secs: *uptime_secs,
        }),
        _ => None,
    }
}

/// Resource request sent to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequest {
    /// CPU cores needed
    pub cpu_cores: u32,
    /// RAM in GB
    pub ram_gb: f32,
    /// GPU VRAM in GB (if applicable)
    pub gpu_vram_gb: Option<f32>,
    /// Concurrent requests expected
    pub concurrent_requests: u32,
    /// Model names this allocation is for
    pub model_names: Vec<String>,
}

/// Resource allocation granted by daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    /// Unique allocation ID
    pub id: String,
    /// CPU cores allocated
    pub cpu_cores: u32,
    /// RAM allocated (GB)
    pub ram_gb: f32,
    /// GPU VRAM allocated (GB)
    pub gpu_vram_gb: Option<f32>,
    /// cgroup path for CPU — le daemon n'en fournit pas : toujours absent.
    pub cpu_cgroup: Option<String>,
    /// cgroup path for memory — le daemon n'en fournit pas : toujours absent.
    pub memory_cgroup: Option<String>,
    /// Expiration time (if any)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Ce que le daemon rapporte : l'allocation courante, pas l'état de la machine.
///
/// Le protocole v1 n'expose ni la RAM totale, ni les cœurs libres, ni la liste
/// des GPU. Les rapporter ici serait les inventer — donc ce type ne les
/// contient pas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationStatus {
    /// Palier courant du daemon
    pub tier: String,
    /// Cœurs alloués
    pub cpu_cores: u32,
    /// RAM allouée, en octets
    pub ram_bytes: u64,
    /// VRAM libre, en Mio
    pub gpu_free_vram_mb: u32,
    /// Plafond CPU du palier, en pourcentage
    pub max_cpu_percent: u8,
    /// Plafond RAM du palier, en pourcentage
    pub max_ram_percent: u8,
    /// Le daemon est-il en train de réduire son allocation ?
    pub shrinking: bool,
    /// Durée de fonctionnement, en secondes
    pub uptime_secs: u64,
}
