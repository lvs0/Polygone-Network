//! Daemon integration for Petals — resource allocation via `polygoned`

use crate::types::ModelRequirements;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Client for communicating with the Polygone daemon (`polygoned`)
pub struct PetalsDaemonClient {
    socket_path: String,
    reader: Option<BufReader<tokio::net::unix::OwnedReadHalf>>,
    writer: Option<tokio::net::unix::OwnedWriteHalf>,
    allocation_id: Option<String>,
}

impl PetalsDaemonClient {
    /// Connect to the daemon
    pub async fn connect() -> Result<Self> {
        let socket_path = std::env::var("POLYGONE_DAEMON_SOCKET")
            .unwrap_or_else(|_| "/tmp/polygoned.sock".to_string());

        Ok(Self {
            socket_path,
            reader: None,
            writer: None,
            allocation_id: None,
        })
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

    /// Send a request to the daemon
    async fn send_request(&mut self, request: DaemonRequest) -> Result<DaemonResponse> {
        self.ensure_connected().await?;

        let request_json = serde_json::to_string(&request)?;
        let writer = self.writer.as_mut().unwrap();
        writer.write_all(request_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        let reader = self.reader.as_mut().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        let response: DaemonResponse = serde_json::from_str(&line)?;
        Ok(response)
    }

    /// Request resource allocation for Petals models
    pub async fn request_allocation(
        &mut self,
        requirements: ResourceRequest,
    ) -> Result<ResourceAllocation> {
        let request = DaemonRequest::AllocateResources {
            client: "petals".to_string(),
            requirements,
        };

        match self.send_request(request).await? {
            DaemonResponse::AllocationGranted { allocation } => {
                self.allocation_id = Some(allocation.id.clone());
                Ok(allocation)
            }
            DaemonResponse::AllocationDenied { reason } => {
                anyhow::bail!("Daemon denied allocation: {}", reason);
            }
            DaemonResponse::Error { message } => {
                anyhow::bail!("Daemon error: {}", message);
            }
            _ => anyhow::bail!("Unexpected response from daemon"),
        }
    }

    /// Release the current allocation
    pub async fn release_allocation(&mut self) -> Result<()> {
        if let Some(allocation_id) = self.allocation_id.take() {
            let request = DaemonRequest::ReleaseAllocation { allocation_id };
            match self.send_request(request).await? {
                DaemonResponse::Released => Ok(()),
                DaemonResponse::Error { message } => {
                    anyhow::bail!("Failed to release: {}", message)
                }
                _ => anyhow::bail!("Unexpected response"),
            }
        } else {
            Ok(())
        }
    }

    /// Get current system resources from daemon
    pub async fn get_system_resources(&mut self) -> Result<SystemResources> {
        let request = DaemonRequest::GetSystemResources;
        match self.send_request(request).await? {
            DaemonResponse::SystemResources(resources) => Ok(resources),
            DaemonResponse::Error { message } => anyhow::bail!("Error: {}", message),
            _ => anyhow::bail!("Unexpected response"),
        }
    }

    /// Register a model with the daemon for tracking
    pub async fn register_model(
        &mut self,
        model: &str,
        requirements: &ModelRequirements,
    ) -> Result<()> {
        let request = DaemonRequest::RegisterModel {
            client: "petals".to_string(),
            model: model.to_string(),
            requirements: requirements.clone(),
        };
        match self.send_request(request).await? {
            DaemonResponse::ModelRegistered => Ok(()),
            DaemonResponse::Error { message } => {
                anyhow::bail!("Failed to register model: {}", message)
            }
            _ => anyhow::bail!("Unexpected response"),
        }
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
    /// cgroup path for CPU
    pub cpu_cgroup: Option<String>,
    /// cgroup path for memory
    pub memory_cgroup: Option<String>,
    /// Expiration time (if any)
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// System resources reported by daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    /// Total CPU cores
    pub total_cpu_cores: u32,
    /// Available CPU cores
    pub available_cpu_cores: u32,
    /// Total RAM (GB)
    pub total_ram_gb: f32,
    /// Available RAM (GB)
    pub available_ram_gb: f32,
    /// Total GPU VRAM (GB)
    pub total_gpu_vram_gb: Option<f32>,
    /// Available GPU VRAM (GB)
    pub available_gpu_vram_gb: Option<f32>,
    /// GPU info
    pub gpus: Vec<GpuInfo>,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub index: u32,
    pub name: String,
    pub vram_total_gb: f32,
    pub vram_free_gb: f32,
    pub compute_capability: Option<String>,
}

/// Daemon request types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonRequest {
    AllocateResources {
        client: String,
        requirements: ResourceRequest,
    },
    ReleaseAllocation {
        allocation_id: String,
    },
    GetSystemResources,
    RegisterModel {
        client: String,
        model: String,
        requirements: ModelRequirements,
    },
    Heartbeat {
        client: String,
        allocation_id: Option<String>,
    },
}

/// Daemon response types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonResponse {
    AllocationGranted { allocation: ResourceAllocation },
    AllocationDenied { reason: String },
    Released,
    SystemResources(SystemResources),
    ModelRegistered,
    Error { message: String },
    Pong,
}
