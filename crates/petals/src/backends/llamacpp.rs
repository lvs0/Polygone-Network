//! llama.cpp / candle native inference backend (placeholder)

use crate::backends::{BackendCapabilities, BackendInfo};
use crate::types::{
    BackendType, ChatRequest, ChatResponse, DeviceType, InferenceRequest, InferenceResponse,
    ModelInfo, ModelSource,
};
use anyhow::Result;
use futures::Stream;

/// llama.cpp / candle backend (placeholder implementation)
#[derive(Debug)]
pub struct LlamaCppBackend {
    // TODO: Add candle model, tokenizer, etc.
}

impl LlamaCppBackend {
    /// Create a new llama.cpp backend
    pub async fn new() -> Result<Self> {
        // TODO: Initialize candle model from GGUF
        Err(anyhow::anyhow!(
            "llama-cpp backend not yet implemented. Enable 'llama-cpp' feature and implement."
        ))
    }

    pub fn backend_type(&self) -> BackendType {
        BackendType::LlamaCpp
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![])
    }

    pub async fn generate(&self, _request: InferenceRequest) -> Result<InferenceResponse> {
        Err(anyhow::anyhow!("llama-cpp backend not yet implemented"))
    }

    pub async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
        Err(anyhow::anyhow!("llama-cpp backend not yet implemented"))
    }

    pub async fn stream(
        &self,
        _request: InferenceRequest,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        Err(anyhow::anyhow!("llama-cpp backend not yet implemented"))
    }

    pub async fn pull_model(&self, _name: &str, _source: ModelSource) -> Result<()> {
        Err(anyhow::anyhow!("llama-cpp backend not yet implemented"))
    }

    pub async fn health_check(&self) -> Result<bool> {
        Ok(false)
    }

    pub fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "llama.cpp / candle".to_string(),
            version: None,
            supported_devices: vec![
                DeviceType::Cpu,
                DeviceType::Cuda,
                DeviceType::Metal,
                DeviceType::Auto,
            ],
            capabilities: BackendCapabilities {
                chat: true,
                completion: true,
                tools: false,
                vision: false,
                streaming: true,
                embeddings: true,
                parallel_requests: false,
                continuous_batching: false,
                paged_attention: false,
                speculative_decoding: false,
            },
        }
    }
}
