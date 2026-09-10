//! vLLM OpenAI-compatible API backend

use crate::backends::{
    generation_config_to_json, http::create_client, BackendCapabilities, BackendInfo,
};
use crate::types::{
    BackendType, ChatRequest, ChatResponse, DeviceType, InferenceRequest, InferenceResponse,
    ModelCapabilities, ModelInfo, ModelSource,
};
use anyhow::Result;
use futures::{Stream, TryStreamExt};
use reqwest::Client;
use std::time::Instant;

/// vLLM backend using OpenAI-compatible API
#[derive(Debug)]
pub struct VllmBackend {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl VllmBackend {
    /// Create a new vLLM backend
    pub async fn new() -> Result<Self> {
        let base_url =
            std::env::var("POLYGONE_VLLM_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".into());
        let api_key = std::env::var("POLYGONE_VLLM_API_KEY").ok();

        let client = create_client(120)?;

        let backend = Self {
            client,
            base_url,
            api_key,
        };
        backend.health_check().await?;

        Ok(backend)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    fn auth_header(&self) -> Option<String> {
        self.api_key.as_ref().map(|k| format!("Bearer {}", k))
    }

    pub fn backend_type(&self) -> BackendType {
        BackendType::Vllm
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut request = self.client.get(self.url("/v1/models"));
        if let Some(auth) = self.auth_header() {
            request = request.header("Authorization", auth);
        }
        let response = request.send().await?;
        let data: serde_json::Value = response.json().await?;

        let models = data
            .get("data")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();

                        Some(ModelInfo {
                            name: id.clone(),
                            display_name: None,
                            source: ModelSource::Vllm,
                            parameters: extract_parameters(&id),
                            quantization: None,
                            context_window: m
                                .get("context_window")
                                .and_then(|c| c.as_u64())
                                .map(|c| c as u32),
                            size_gb: None,
                            supported_devices: vec![
                                DeviceType::Cuda,
                                DeviceType::Cpu,
                                DeviceType::Auto,
                            ],
                            capabilities: ModelCapabilities {
                                chat: true,
                                completion: true,
                                tools: true,
                                vision: id.contains("vision") || id.contains("llava"),
                                streaming: true,
                                embeddings: true,
                                max_concurrent: Some(128),
                            },
                            metadata: m.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }

    pub async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse> {
        let model = request
            .model
            .ok_or_else(|| anyhow::anyhow!("Model required"))?;
        let start = Instant::now();

        let mut payload = serde_json::json!({
            "model": model,
            "prompt": request.prompt,
            "stream": false,
        });
        // Merge generation config
        if let Some(config) = generation_config_to_json(&request.config).as_object() {
            for (k, v) in config {
                payload[k] = v.clone();
            }
        }

        let mut req = self.client.post(self.url("/v1/completions")).json(&payload);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let response = req.send().await?;
        let elapsed = start.elapsed().as_millis() as u64;
        let data: serde_json::Value = response.json().await?;

        let choice = data
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first());
        let text = choice
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let tokens = data
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let prompt_tokens = data
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        Ok(InferenceResponse {
            text,
            model,
            tokens_generated: tokens,
            ttft_ms: None,
            total_time_ms: elapsed,
            tokens_per_second: crate::backends::calculate_tokens_per_second(tokens, elapsed),
            finish_reason: choice
                .and_then(|c| c.get("finish_reason"))
                .and_then(|r| r.as_str())
                .map(|s| s.to_string()),
            usage: Some(crate::types::Usage {
                prompt_tokens,
                completion_tokens: tokens,
                total_tokens: prompt_tokens + tokens,
            }),
        })
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let model = request
            .model
            .ok_or_else(|| anyhow::anyhow!("Model required"))?;

        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|m| {
                let mut msg = serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(tool_calls) = &m.tool_calls {
                    msg["tool_calls"] = serde_json::to_value(tool_calls).unwrap_or_default();
                }
                if let Some(tool_call_id) = &m.tool_call_id {
                    msg["tool_call_id"] = serde_json::to_value(tool_call_id).unwrap_or_default();
                }
                msg
            })
            .collect();

        let mut payload = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });
        if let Some(config) = generation_config_to_json(&request.config).as_object() {
            for (k, v) in config {
                payload[k] = v.clone();
            }
        }

        if let Some(tools) = &request.tools {
            payload["tools"] = serde_json::to_value(tools).unwrap_or_default();
        }
        if let Some(tool_choice) = &request.tool_choice {
            payload["tool_choice"] = serde_json::to_value(tool_choice).unwrap_or_default();
        }

        let mut req = self
            .client
            .post(self.url("/v1/chat/completions"))
            .json(&payload);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let response = req.send().await?;
        let data: serde_json::Value = response.json().await?;

        let choice = data
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first());
        let message = choice
            .and_then(|c| c.get("message"))
            .cloned()
            .unwrap_or_default();
        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let tool_calls = message
            .get("tool_calls")
            .and_then(|t| serde_json::from_value(t.clone()).ok());

        let tokens = data
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let prompt_tokens = data
            .get("usage")
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        Ok(ChatResponse {
            message: crate::types::ChatMessage {
                role: "assistant".to_string(),
                content,
                tool_calls,
                tool_call_id: None,
            },
            model,
            finish_reason: choice
                .and_then(|c| c.get("finish_reason"))
                .and_then(|r| r.as_str())
                .map(|s| s.to_string()),
            usage: Some(crate::types::Usage {
                prompt_tokens,
                completion_tokens: tokens,
                total_tokens: prompt_tokens + tokens,
            }),
        })
    }

    pub async fn stream(
        &self,
        request: InferenceRequest,
    ) -> Result<Box<dyn Stream<Item = Result<String>> + Send + Unpin>> {
        let model = request
            .model
            .ok_or_else(|| anyhow::anyhow!("Model required"))?;

        let mut payload = serde_json::json!({
            "model": model,
            "prompt": request.prompt,
            "stream": true,
        });
        if let Some(config) = generation_config_to_json(&request.config).as_object() {
            for (k, v) in config {
                payload[k] = v.clone();
            }
        }

        let mut req = self.client.post(self.url("/v1/completions")).json(&payload);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let response = req.send().await?;
        let stream = response.bytes_stream();
        let stream = stream
            .map_err(anyhow::Error::from)
            .try_filter_map(|bytes| async move {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return Ok(None);
                        }
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(choices) = json.get("choices").and_then(|c| c.as_array()) {
                                if let Some(choice) = choices.first() {
                                    if let Some(text) = choice.get("text").and_then(|t| t.as_str())
                                    {
                                        return Ok(Some(text.to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None)
            });

        Ok(Box::new(Box::pin(stream)))
    }

    pub async fn pull_model(&self, _name: &str, _source: ModelSource) -> Result<()> {
        anyhow::bail!("vLLM backend does not support model pulling. Use the vLLM server directly.");
    }

    pub async fn health_check(&self) -> Result<bool> {
        let mut request = self.client.get(self.url("/health"));
        if let Some(auth) = self.auth_header() {
            request = request.header("Authorization", auth);
        }
        match request.send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "vLLM".to_string(),
            version: None,
            supported_devices: vec![DeviceType::Cuda, DeviceType::Cpu, DeviceType::Auto],
            capabilities: BackendCapabilities {
                chat: true,
                completion: true,
                tools: true,
                vision: true,
                streaming: true,
                embeddings: true,
                parallel_requests: true,
                continuous_batching: true,
                paged_attention: true,
                speculative_decoding: true,
            },
        }
    }
}

/// Extract parameter count from model name
fn extract_parameters(name: &str) -> Option<String> {
    let name_lower = name.to_lowercase();
    for part in name_lower.split(['/', '-', '_', '.']) {
        if part.ends_with('b') && part.len() > 1 {
            let num_part = &part[..part.len() - 1];
            if num_part.parse::<f32>().is_ok() {
                return Some(format!("{}B", num_part.to_uppercase()));
            }
        }
    }
    None
}
