//! Backend implementations for different inference engines

pub mod llamacpp;
pub mod mock;
pub mod ollama;
pub mod vllm;

use crate::types::{
    BackendType, ChatRequest, ChatResponse, DeviceType, GenerationConfig, InferenceRequest,
    InferenceResponse, ModelInfo, ModelSource,
};
use anyhow::Result;
use futures::Stream;

/// Backend enum for dyn-compatible dispatch
#[derive(Debug)]
pub enum InferenceBackend {
    Ollama(ollama::OllamaBackend),
    Vllm(vllm::VllmBackend),
    LlamaCpp(llamacpp::LlamaCppBackend),
    Mock(mock::MockBackend),
}

impl InferenceBackend {
    pub fn backend_type(&self) -> BackendType {
        match self {
            Self::Ollama(b) => b.backend_type(),
            Self::Vllm(b) => b.backend_type(),
            Self::LlamaCpp(b) => b.backend_type(),
            Self::Mock(b) => b.backend_type(),
        }
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        match self {
            Self::Ollama(b) => b.list_models().await,
            Self::Vllm(b) => b.list_models().await,
            Self::LlamaCpp(b) => b.list_models().await,
            Self::Mock(b) => b.list_models().await,
        }
    }

    pub async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        match self {
            Self::Ollama(b) => b.generate(request).await,
            Self::Vllm(b) => b.generate(request).await,
            Self::LlamaCpp(b) => b.generate(request).await,
            Self::Mock(b) => b.generate(request).await,
        }
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        match self {
            Self::Ollama(b) => b.chat(request).await,
            Self::Vllm(b) => b.chat(request).await,
            Self::LlamaCpp(b) => b.chat(request).await,
            Self::Mock(b) => b.chat(request).await,
        }
    }

    pub async fn stream(
        &self,
        request: InferenceRequest,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        match self {
            Self::Ollama(b) => b.stream(request).await,
            Self::Vllm(b) => b.stream(request).await,
            Self::LlamaCpp(b) => b.stream(request).await,
            Self::Mock(b) => b.stream(request).await,
        }
    }

    pub async fn pull_model(&self, name: &str, source: ModelSource) -> Result<()> {
        match self {
            Self::Ollama(b) => b.pull_model(name, source).await,
            Self::Vllm(b) => b.pull_model(name, source).await,
            Self::LlamaCpp(b) => b.pull_model(name, source).await,
            Self::Mock(b) => b.pull_model(name, source).await,
        }
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self {
            Self::Ollama(b) => b.health_check().await,
            Self::Vllm(b) => b.health_check().await,
            Self::LlamaCpp(b) => b.health_check().await,
            Self::Mock(b) => b.health_check().await,
        }
    }

    pub fn info(&self) -> BackendInfo {
        match self {
            Self::Ollama(b) => b.info(),
            Self::Vllm(b) => b.info(),
            Self::LlamaCpp(b) => b.info(),
            Self::Mock(b) => b.info(),
        }
    }
}

/// Backend information
#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub name: String,
    pub version: Option<String>,
    pub supported_devices: Vec<DeviceType>,
    pub capabilities: BackendCapabilities,
}

/// Backend capabilities
#[derive(Debug, Clone, Default)]
pub struct BackendCapabilities {
    pub chat: bool,
    pub completion: bool,
    pub tools: bool,
    pub vision: bool,
    pub streaming: bool,
    pub embeddings: bool,
    pub parallel_requests: bool,
    pub continuous_batching: bool,
    pub paged_attention: bool,
    pub speculative_decoding: bool,
}

/// Common HTTP client utilities
pub mod http {
    use anyhow::Result;
    use reqwest::Client;

    pub fn create_client(timeout_secs: u64) -> Result<Client> {
        Ok(Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()?)
    }
}

/// Helper to convert GenerationConfig to backend-specific format
pub fn generation_config_to_json(config: &GenerationConfig) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    if let Some(v) = config.max_tokens {
        map.insert("max_tokens".into(), v.into());
    }
    if let Some(v) = config.temperature {
        map.insert("temperature".into(), v.into());
    }
    if let Some(v) = config.top_p {
        map.insert("top_p".into(), v.into());
    }
    if let Some(v) = config.top_k {
        map.insert("top_k".into(), v.into());
    }
    if let Some(v) = config.repetition_penalty {
        map.insert("repetition_penalty".into(), v.into());
    }
    if let Some(v) = &config.stop {
        map.insert("stop".into(), v.clone().into());
    }
    if let Some(v) = config.seed {
        map.insert("seed".into(), v.into());
    }
    if let Some(v) = config.n {
        map.insert("n".into(), v.into());
    }

    serde_json::Value::Object(map)
}

/// Calculate tokens per second from timing
pub fn calculate_tokens_per_second(tokens: u32, elapsed_ms: u64) -> f32 {
    if elapsed_ms == 0 {
        0.0
    } else {
        (tokens as f32) / (elapsed_ms as f32 / 1000.0)
    }
}
