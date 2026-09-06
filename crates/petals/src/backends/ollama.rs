//! Ollama HTTP API backend

use reqwest::Client;
use crate::types::{
    BackendType, DeviceType, InferenceRequest, InferenceResponse, ChatRequest, ChatResponse,
    ModelInfo, ModelSource, GenerationConfig, ModelCapabilities,
};
use crate::backends::{BackendInfo, BackendCapabilities, http::create_client, generation_config_to_json};
use anyhow::Result;
use futures::{Stream, StreamExt};
use std::time::Instant;

/// Ollama backend using HTTP API
#[derive(Debug)]
pub struct OllamaBackend {
    client: Client,
    base_url: String,
}

impl OllamaBackend {
    /// Create a new Ollama backend
    pub async fn new() -> Result<Self> {
        let base_url = std::env::var("POLYGONE_OLLAMA_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:11434".into());

        let client = create_client(120)?;

        // Test connection
        let backend = Self { client, base_url };
        backend.health_check().await?;

        Ok(backend)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    pub fn backend_type(&self) -> BackendType {
        BackendType::Ollama
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self.client.get(self.url("/api/tags")).send().await?;
        let data: serde_json::Value = response.json().await?;

        let models = data.get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter().filter_map(|m| {
                    let name = m.get("name")?.as_str()?.to_string();
                    let size = m.get("size").and_then(|s| s.as_u64());
                    let modified = m.get("modified_at").and_then(|d| d.as_str());

                    Some(ModelInfo {
                        name: name.clone(),
                        display_name: None,
                        source: ModelSource::Ollama,
                        parameters: extract_parameters(&name),
                        quantization: extract_quantization(&name),
                        context_window: None,
                        size_gb: size.map(|s| s as f32 / 1_073_741_824.0),
                        supported_devices: vec![DeviceType::Cpu, DeviceType::Auto],
                        capabilities: ModelCapabilities {
                            chat: true,
                            completion: true,
                            tools: false,
                            vision: name.contains("llava") || name.contains("vision"),
                            streaming: true,
                            embeddings: true,
                            max_concurrent: Some(4),
                        },
                        metadata: m.clone(),
                    })
                }).collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    pub async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        let model = request.model.ok_or_else(|| anyhow::anyhow!("Model required"))?;
        let start = Instant::now();

        let payload = serde_json::json!({
            "model": model,
            "prompt": request.prompt,
            "stream": false,
            "options": generation_config_to_json(&request.config),
        });

        let response = self.client
            .post(self.url("/api/generate"))
            .json(&payload)
            .send()
            .await?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data: serde_json::Value = response.json().await?;

        let text = data.get("response")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let tokens = data.get("eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
        let ttft = data.get("eval_duration").and_then(|d| d.as_u64()).map(|d| d / 1_000_000);

        Ok(InferenceResponse {
            text,
            model,
            tokens_generated: tokens,
            ttft_ms: ttft,
            total_time_ms: elapsed,
            tokens_per_second: crate::backends::calculate_tokens_per_second(tokens, elapsed),
            finish_reason: data.get("done_reason").and_then(|r| r.as_str()).map(|s| s.to_string()),
            usage: Some(crate::types::Usage {
                prompt_tokens: data.get("prompt_eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                completion_tokens: tokens,
                total_tokens: data.get("prompt_eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32 + tokens,
            }),
        })
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = request.model.ok_or_else(|| anyhow::anyhow!("Model required"))?;
        let start = Instant::now();

        let messages: Vec<serde_json::Value> = request.messages.iter().map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
            })
        }).collect();

        let payload = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "options": generation_config_to_json(&request.config),
        });

        let response = self.client
            .post(self.url("/api/chat"))
            .json(&payload)
            .send()
            .await?;

        let elapsed = start.elapsed().as_millis() as u64;
        let data: serde_json::Value = response.json().await?;

        let message = data.get("message").cloned().unwrap_or_default();
        let content = message.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();

        let tokens = data.get("eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32;

        Ok(ChatResponse {
            message: crate::types::ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls: None,
                tool_call_id: None,
            },
            model,
            finish_reason: data.get("done_reason").and_then(|r| r.as_str()).map(|s| s.to_string()),
            usage: Some(crate::types::Usage {
                prompt_tokens: data.get("prompt_eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
                completion_tokens: tokens,
                total_tokens: data.get("prompt_eval_count").and_then(|t| t.as_u64()).unwrap_or(0) as u32 + tokens,
            }),
        })
    }

    pub async fn stream(&self, request: InferenceRequest) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        let model = request.model.ok_or_else(|| anyhow::anyhow!("Model required"))?;

        let payload = serde_json::json!({
            "model": model,
            "prompt": request.prompt,
            "stream": true,
            "options": generation_config_to_json(&request.config),
        });

        let response = self.client
            .post(self.url("/api/generate"))
            .json(&payload)
            .send()
            .await?;

        let stream = response.bytes_stream();
        let stream = stream.map(|chunk| {
            chunk.map_err(anyhow::Error::from).and_then(|bytes| {
                let text = String::from_utf8_lossy(&bytes);
                // Parse SSE format
                for line in text.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if data == "[DONE]" {
                            return Ok(None);
                        }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(response_text) = json.get("response").and_then(|r| r.as_str()) {
                                return Ok(Some(response_text.to_string()));
                            }
                        }
                    }
                }
                Ok(None)
            })
        })
        .filter_map(|r| async move { r.transpose() });

        Ok(Box::new(Box::pin(stream)))
    }

    pub async fn pull_model(&self, name: &str, source: ModelSource) -> Result<()> {
        if source != ModelSource::Ollama {
            anyhow::bail!("Ollama backend only supports Ollama model source");
        }

        let payload = serde_json::json!({
            "name": name,
            "stream": false,
        });

        let response = self.client
            .post(self.url("/api/pull"))
            .json(&payload)
            .send()
            .await?;

        let data: serde_json::Value = response.json().await?;

        if let Some(error) = data.get("error").and_then(|e| e.as_str()) {
            anyhow::bail!("Ollama pull error: {}", error);
        }

        Ok(())
    }

    pub async fn health_check(&self) -> Result<bool> {
        match self.client.get(self.url("/api/tags")).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "Ollama".to_string(),
            version: None,
            supported_devices: vec![DeviceType::Cpu, DeviceType::Auto],
            capabilities: BackendCapabilities {
                chat: true,
                completion: true,
                tools: false,
                vision: true,
                streaming: true,
                embeddings: true,
                parallel_requests: true,
                continuous_batching: false,
                paged_attention: false,
                speculative_decoding: false,
            },
        }
    }
}

/// Extract parameter count from model name (e.g., "llama3.1:7b" -> "7B")
fn extract_parameters(name: &str) -> Option<String> {
    let name_lower = name.to_lowercase();
    for part in name_lower.split([':', '-', '_', '.']) {
        if part.ends_with('b') && part.len() > 1 {
            let num_part = &part[..part.len()-1];
            if num_part.parse::<f32>().is_ok() {
                return Some(format!("{}B", num_part.to_uppercase()));
            }
        }
    }
    None
}

/// Extract quantization from model name (e.g., "llama3.1:7b-q4_k_m" -> "Q4_K_M")
fn extract_quantization(name: &str) -> Option<String> {
    let name_lower = name.to_lowercase();
    for part in name_lower.split([':', '-', '_', '.']) {
        if part.starts_with('q') && part.len() > 1 {
            let rest = &part[1..];
            if rest.chars().next().unwrap_or(' ').is_ascii_digit() {
                return Some(part.to_uppercase());
            }
        }
    }
    None
}