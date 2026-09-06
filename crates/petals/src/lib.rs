//! Petals — Local AI Inference Engine for Agentic Workloads
//!
//! This crate provides a unified interface for running LLMs locally on CPU/GPU
//! with multiple backends:
//! - **Ollama** (default): Simple HTTP API, good for quick prototyping
//! - **vLLM**: High-throughput OpenAI-compatible API, PagedAttention, continuous batching
//! - **llama.cpp / candle**: Native Rust inference, GGUF support, no Python dependency
//!
//! Design goals:
//! - Zero cloud dependency — models run on your hardware
//! - CPU-first architecture — agentic workloads are CPU-friendly (conditional logic, branching, direct memory access)
//! - Benchmarkable — built-in criterion benches for tokens/s, latency, memory
//! - Daemon-integrated — exposes CPU/GPU/memory requirements to `polygoned` for allocation
//! - Multi-arch — x86_64, ARM64 (Apple Silicon, NVIDIA Grace, Alibaba XuanTie C950 RISC-V)
//!
//! # Quick Start
//!
//! ```no_run
//! use polygone_petals::{PetalsEngine, BackendType, InferenceRequest};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let engine = PetalsEngine::new(BackendType::Ollama).await?;
//!     let response = engine.generate(InferenceRequest::new("What is ML-KEM-1024?")).await?;
//!     println!("{}", response.text);
//!     Ok(())
//! }
//! ```
//!
//! # Benchmarking
//!
//! Run CPU inference benchmarks:
//! ```bash
//! cargo bench -p polygone-petals --bench inference_bench --features bench
//! ```
//!
//! # Daemon Integration
//!
//! Petals registers its resource requirements with `polygoned`:
//! - CPU cores needed (based on model size + concurrent requests)
//! - RAM (model weights + KV cache + overhead)
//! - GPU VRAM (if using GPU offload)
//!
//! The daemon allocates resources via cgroups/systemd and reports back the allocation.

pub mod backends;
pub mod benchmarks;
pub mod daemon;
pub mod models;
pub mod types;

pub use backends::{InferenceBackend};
pub use benchmarks::{BenchmarkConfig, BenchmarkResult, InferenceBench};
pub use daemon::{PetalsDaemonClient, ResourceRequest, ResourceAllocation};
pub use models::{ModelRegistry};
pub use types::{
    CompletionRequest, CompletionResponse, ChatMessage, ChatRequest, ChatResponse,
    InferenceRequest, InferenceResponse, GenerationConfig, BackendType, DeviceType,
    ModelCapabilities, ModelRequirements, ModelInfo, ModelSource,
};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main Petals engine — unified interface for all backends
pub struct PetalsEngine {
    backend: Arc<InferenceBackend>,
    registry: Arc<RwLock<ModelRegistry>>,
    daemon_client: Option<Arc<tokio::sync::Mutex<PetalsDaemonClient>>>,
    config: EngineConfig,
}

/// Configuration for the Petals engine
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub default_model: Option<String>,
    pub max_concurrent_requests: usize,
    pub request_timeout_secs: u64,
    pub enable_benchmarks: bool,
    pub daemon_enabled: bool,
    pub preferred_device: DeviceType,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            default_model: None,
            max_concurrent_requests: 4,
            request_timeout_secs: 120,
            enable_benchmarks: false,
            daemon_enabled: true,
            preferred_device: DeviceType::Cpu,
        }
    }
}

impl PetalsEngine {
    /// Create a new Petals engine with the specified backend
    pub async fn new(backend_type: BackendType) -> Result<Self> {
        Self::with_config(backend_type, EngineConfig::default()).await
    }

    /// Create a new Petals engine with custom configuration
    pub async fn with_config(backend_type: BackendType, config: EngineConfig) -> Result<Self> {
        let backend = match backend_type {
            BackendType::Ollama => InferenceBackend::Ollama(backends::ollama::OllamaBackend::new().await?),
            BackendType::Vllm => InferenceBackend::Vllm(backends::vllm::VllmBackend::new().await?),
            BackendType::LlamaCpp => InferenceBackend::LlamaCpp(backends::llamacpp::LlamaCppBackend::new().await?),
            BackendType::Auto => {
                // Try backends in order of preference
                if let Ok(b) = backends::vllm::VllmBackend::new().await {
                    InferenceBackend::Vllm(b)
                } else if let Ok(b) = backends::ollama::OllamaBackend::new().await {
                    InferenceBackend::Ollama(b)
                } else {
                    InferenceBackend::LlamaCpp(backends::llamacpp::LlamaCppBackend::new().await?)
                }
            }
        };

        let registry = Arc::new(RwLock::new(ModelRegistry::new()));
        let daemon_client = if config.daemon_enabled {
            Some(Arc::new(tokio::sync::Mutex::new(PetalsDaemonClient::connect().await?)))
        } else {
            None
        };

        let engine = Self {
            backend: Arc::new(backend),
            registry,
            daemon_client,
            config,
        };

        // Register models from backend
        engine.refresh_models().await?;

        // Register with daemon if enabled
        if let Some(ref daemon) = engine.daemon_client {
            let requirements = engine.estimate_resource_requirements().await?;
            daemon.lock().await.request_allocation(requirements).await?;
        }

        Ok(engine)
    }

    /// Generate a completion for the given request
    pub async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        let model = request.model.clone().or_else(|| self.config.default_model.clone())
            .ok_or_else(|| anyhow::anyhow!("No model specified and no default configured"))?;

        // Check if model is available
        let registry = self.registry.read().await;
        if !registry.has_model(&model) {
            anyhow::bail!("Model '{}' not found. Available: {:?}", model, registry.list_models());
        }
        drop(registry);

        // Generate via backend
        let response = self.backend.generate(request).await?;

        Ok(response)
    }

    /// Generate a chat completion
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = request.model.clone().or_else(|| self.config.default_model.clone())
            .ok_or_else(|| anyhow::anyhow!("No model specified and no default configured"))?;

        let registry = self.registry.read().await;
        if !registry.has_model(&model) {
            anyhow::bail!("Model '{}' not found", model);
        }
        drop(registry);

        self.backend.chat(request).await
    }

    /// Stream a completion (for real-time output)
    pub async fn stream(&self, request: InferenceRequest) -> Result<impl futures::Stream<Item = Result<String>>> {
        self.backend.stream(request).await
    }

    /// List available models
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.registry.read().await.list_models()
    }

    /// Get model info
    pub async fn model_info(&self, name: &str) -> Option<ModelInfo> {
        self.registry.read().await.get_model(name).cloned()
    }

    /// Refresh model list from backend
    pub async fn refresh_models(&self) -> Result<()> {
        let models = self.backend.list_models().await?;
        let mut registry = self.registry.write().await;
        registry.update(models);
        Ok(())
    }

    /// Pull/download a model
    pub async fn pull_model(&self, name: &str, source: ModelSource) -> Result<()> {
        self.backend.pull_model(name, source).await?;
        self.refresh_models().await
    }

    /// Estimate resource requirements for current models
    pub async fn estimate_resource_requirements(&self) -> Result<ResourceRequest> {
        let registry = self.registry.read().await;
        let models = registry.list_models();

        let mut total_ram_gb = 0.0;
        let mut max_cpu_cores = 0;
        let mut gpu_vram_gb = 0.0;

        for model in &models {
            let req = model.estimate_requirements(self.config.preferred_device);
            total_ram_gb += req.ram_gb;
            max_cpu_cores = max_cpu_cores.max(req.cpu_cores);
            if let Some(vram) = req.gpu_vram_gb {
                gpu_vram_gb += vram;
            }
        }

        // Add overhead for KV cache and concurrent requests
        let concurrent = self.config.max_concurrent_requests as f32;
        total_ram_gb *= 1.0 + (concurrent * 0.1); // 10% per concurrent request

        Ok(ResourceRequest {
            cpu_cores: max_cpu_cores.max(1),
            ram_gb: total_ram_gb.max(1.0),
            gpu_vram_gb: if gpu_vram_gb > 0.0 { Some(gpu_vram_gb) } else { None },
            concurrent_requests: self.config.max_concurrent_requests as u32,
            model_names: models.iter().map(|m| m.name.clone()).collect(),
        })
    }

    /// Run a benchmark
    pub async fn benchmark(&self, config: BenchmarkConfig) -> Result<BenchmarkResult> {
        if !self.config.enable_benchmarks {
            anyhow::bail!("Benchmarks not enabled. Create engine with `enable_benchmarks: true`");
        }

        let bench = InferenceBench::new(self.backend.clone(), config);
        bench.run().await
    }

    /// Get the backend type
    pub fn backend_type(&self) -> BackendType {
        self.backend.backend_type()
    }

    /// Shutdown and release daemon allocation
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(ref daemon) = self.daemon_client {
            daemon.lock().await.release_allocation().await?;
        }
        Ok(())
    }
}

/// Re-export commonly used types
pub mod prelude {
    pub use crate::{
        PetalsEngine, EngineConfig, BackendType, DeviceType,
        InferenceRequest, InferenceResponse, ChatRequest, ChatResponse,
        CompletionRequest, CompletionResponse, GenerationConfig,
        ModelInfo, ModelRegistry, ModelSource,
        ResourceRequest, ResourceAllocation,
        BenchmarkConfig, BenchmarkResult,
    };
}