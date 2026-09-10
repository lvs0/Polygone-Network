//! Mock backend for testing Petals inference engine without external dependencies

use crate::backends::{BackendCapabilities, BackendInfo};
use crate::types::{
    ChatRequest, ChatResponse, DeviceType,
    InferenceRequest, InferenceResponse, ModelCapabilities, ModelInfo,
    ModelSource, Usage,
};
use anyhow::Result;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

/// Mock backend for testing — no external server required
#[derive(Debug, Clone)]
pub struct MockBackend {
    /// Simulated delay in milliseconds
    pub delay_ms: u64,
    /// Fixed response text
    pub response: String,
    /// Simulated tokens per second
    pub tokens_per_second: f32,
    /// Whether to simulate failure
    pub should_fail: bool,
    /// Failure message
    pub fail_message: String,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self {
            delay_ms: 10,
            response: "mock response".to_string(),
            tokens_per_second: 100.0,
            should_fail: false,
            fail_message: "mock failure".to_string(),
        }
    }
}

impl MockBackend {
    /// Create a new mock backend with custom response
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            ..Default::default()
        }
    }

    /// Create a mock backend that fails
    pub fn failing(message: impl Into<String>) -> Self {
        Self {
            should_fail: true,
            fail_message: message.into(),
            ..Default::default()
        }
    }
}

/// Stream implementation for mock backend
pub struct MockStream {
    chunks: Vec<String>,
    index: usize,
    #[allow(dead_code)]
    delay_ms: u64,
    woken: bool,
}

impl MockStream {
    fn new(response: &str, delay_ms: u64) -> Self {
        let chunks: Vec<String> = response
            .split_whitespace()
            .map(|s| format!("{} ", s))
            .collect();
        Self {
            chunks,
            index: 0,
            delay_ms,
            woken: false,
        }
    }
}

impl Stream for MockStream {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.index >= self.chunks.len() {
            return Poll::Ready(None);
        }

        if !self.woken {
            self.woken = true;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        self.woken = false;
        let chunk = self.chunks[self.index].clone();
        self.index += 1;
        Poll::Ready(Some(Ok(chunk)))
    }
}

/// Mock backend for testing — no external server required
impl MockBackend {
    /// List available models (mock)
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.fail_message));
        }
        Ok(vec![
            ModelInfo {
                name: "mock-model:7b".to_string(),
                display_name: Some("Mock Model 7B".to_string()),
                source: ModelSource::LocalGguf,
                parameters: Some("7B".to_string()),
                quantization: Some("Q4_K_M".to_string()),
                context_window: Some(4096),
                size_gb: Some(4.0),
                supported_devices: vec![DeviceType::Cpu],
                capabilities: ModelCapabilities {
                    chat: true,
                    completion: true,
                    tools: false,
                    vision: false,
                    streaming: true,
                    embeddings: true,
                    max_concurrent: Some(4),
                },
                metadata: serde_json::json!({}),
            },
        ])
    }

    /// Generate a completion (mock)
    pub async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.fail_message));
        }

        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;

        let tokens = self.response.split_whitespace().count() as u32;
        Ok(InferenceResponse {
            text: self.response.clone(),
            model: request.model.unwrap_or_else(|| "mock-model".to_string()),
            tokens_generated: tokens,
            ttft_ms: Some(self.delay_ms),
            total_time_ms: self.delay_ms,
            tokens_per_second: self.tokens_per_second,
            finish_reason: Some("stop".to_string()),
            usage: Some(Usage {
                prompt_tokens: request.prompt.split_whitespace().count() as u32,
                completion_tokens: tokens,
                total_tokens: tokens + request.prompt.split_whitespace().count() as u32,
            }),
        })
    }

    /// Chat completion (mock)
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.fail_message));
        }

        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;

        let content = self.response.clone();
        Ok(ChatResponse {
            message: crate::types::ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: None,
                tool_call_id: None,
            },
            model: request.model.unwrap_or_else(|| "mock-model".to_string()),
            finish_reason: Some("stop".to_string()),
            usage: Some(Usage {
                prompt_tokens: request
                    .messages
                    .iter()
                    .map(|m| m.content.split_whitespace().count() as u32)
                    .sum(),
                completion_tokens: self.response.split_whitespace().count() as u32,
                total_tokens: 0,
            }),
        })
    }

    /// Stream a completion (mock)
    pub async fn stream(
        &self,
        _request: InferenceRequest,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.fail_message));
        }

        let stream = MockStream::new(&self.response, self.delay_ms);
        Ok(Box::new(stream))
    }

    /// Pull/download a model (mock)
    pub async fn pull_model(&self, _name: &str, _source: ModelSource) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("{}", self.fail_message));
        }
        Ok(())
    }

    /// Health check (mock)
    pub async fn health_check(&self) -> Result<bool> {
        Ok(!self.should_fail)
    }

    /// Backend info (mock)
    pub fn info(&self) -> crate::backends::BackendInfo {
        BackendInfo {
            name: "Mock".to_string(),
            version: Some("0.1.0".to_string()),
            supported_devices: vec![DeviceType::Cpu],
            capabilities: BackendCapabilities {
                chat: true,
                completion: true,
                tools: false,
                vision: false,
                streaming: true,
                embeddings: true,
                parallel_requests: true,
                continuous_batching: false,
                paged_attention: false,
                speculative_decoding: false,
            },
        }
    }

    /// Backend type for mock
    pub fn backend_type(&self) -> crate::types::BackendType {
        crate::types::BackendType::Ollama
    }
}