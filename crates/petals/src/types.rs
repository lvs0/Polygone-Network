//! Core types for Petals inference engine

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Backend type for inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendType {
    /// Ollama HTTP API
    Ollama,
    /// vLLM OpenAI-compatible API
    Vllm,
    /// llama.cpp / candle native inference
    LlamaCpp,
    /// Auto-detect best available backend
    Auto,
}

/// Device type for inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceType {
    /// CPU inference
    Cpu,
    /// CUDA GPU
    Cuda,
    /// Metal (Apple Silicon)
    Metal,
    /// Vulkan
    Vulkan,
    /// Auto-detect
    Auto,
}

/// Generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 = deterministic)
    pub temperature: Option<f32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Top-k sampling
    pub top_k: Option<u32>,
    /// Repetition penalty
    pub repetition_penalty: Option<f32>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Seed for reproducibility
    pub seed: Option<u64>,
    /// Number of completions to generate
    pub n: Option<u32>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: Some(2048),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(50),
            repetition_penalty: Some(1.1),
            stop: None,
            seed: None,
            n: Some(1),
        }
    }
}

/// Inference request (completion-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// Prompt text
    pub prompt: String,
    /// Model name
    pub model: Option<String>,
    /// Generation config
    #[serde(flatten)]
    pub config: GenerationConfig,
    /// Stream response
    pub stream: bool,
    /// Additional backend-specific parameters
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl InferenceRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            model: None,
            config: GenerationConfig::default(),
            stream: false,
            extra: HashMap::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_config(mut self, config: GenerationConfig) -> Self {
        self.config = config;
        self
    }

    pub fn streaming(mut self) -> Self {
        self.stream = true;
        self
    }
}

/// Inference response (completion-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    /// Generated text
    pub text: String,
    /// Model used
    pub model: String,
    /// Tokens generated
    pub tokens_generated: u32,
    /// Time to first token (ms)
    pub ttft_ms: Option<u64>,
    /// Total generation time (ms)
    pub total_time_ms: u64,
    /// Tokens per second
    pub tokens_per_second: f32,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Usage statistics
    pub usage: Option<Usage>,
}

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens
    pub prompt_tokens: u32,
    /// Completion tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role (system, user, assistant, tool)
    pub role: String,
    /// Content
    pub content: String,
    /// Tool calls (for assistant)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (for tool messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

/// Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Chat request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Messages
    pub messages: Vec<ChatMessage>,
    /// Model name
    pub model: Option<String>,
    /// Generation config
    #[serde(flatten)]
    pub config: GenerationConfig,
    /// Stream response
    pub stream: bool,
    /// Tools available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool choice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

/// Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub r#type: String,
    pub function: FunctionDef,
}

/// Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Tool choice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto(String),
    Required(String),
    Function { r#type: String, function: FunctionChoice },
}

/// Function choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionChoice {
    pub name: String,
}

/// Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Message
    pub message: ChatMessage,
    /// Model used
    pub model: String,
    /// Finish reason
    pub finish_reason: Option<String>,
    /// Usage statistics
    pub usage: Option<Usage>,
}

/// Completion request (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: String,
    #[serde(flatten)]
    pub config: GenerationConfig,
    pub stream: bool,
}

/// Completion response (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: Option<Usage>,
}

/// Completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    pub index: u32,
    pub text: String,
    pub finish_reason: Option<String>,
}

/// Model source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelSource {
    /// Hugging Face Hub
    HuggingFace,
    /// Ollama library
    Ollama,
    /// Local GGUF file
    LocalGguf,
    /// vLLM model repository
    Vllm,
}

/// Resource requirements for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequirements {
    /// CPU cores needed
    pub cpu_cores: u32,
    /// RAM in GB
    pub ram_gb: f32,
    /// GPU VRAM in GB (if applicable)
    pub gpu_vram_gb: Option<f32>,
    /// Disk space in GB
    pub disk_gb: f32,
    /// Estimated tokens/second on reference hardware
    pub est_tokens_per_sec: Option<f32>,
}

/// Detailed model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model name/identifier
    pub name: String,
    /// Human-readable name
    pub display_name: Option<String>,
    /// Model source
    pub source: ModelSource,
    /// Parameter count (e.g., "7B", "70B")
    pub parameters: Option<String>,
    /// Quantization (e.g., "Q4_K_M", "FP16")
    pub quantization: Option<String>,
    /// Context window size
    pub context_window: Option<u32>,
    /// Model size in GB
    pub size_gb: Option<f32>,
    /// Supported devices
    pub supported_devices: Vec<DeviceType>,
    /// Capabilities
    pub capabilities: ModelCapabilities,
    /// Backend-specific metadata
    pub metadata: serde_json::Value,
}

/// Model capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports chat/completion
    pub chat: bool,
    /// Supports completion
    pub completion: bool,
    /// Supports tool/function calling
    pub tools: bool,
    /// Supports vision/multimodal
    pub vision: bool,
    /// Supports streaming
    pub streaming: bool,
    /// Supports embeddings
    pub embeddings: bool,
    /// Max concurrent requests
    pub max_concurrent: Option<u32>,
}

impl ModelInfo {
    /// Estimate resource requirements for this model on a device
    pub fn estimate_requirements(&self, device: DeviceType) -> ModelRequirements {
        let params = self.parse_parameters().unwrap_or(7.0); // Default 7B
        let quant_factor = self.quantization_factor();

        // Base estimates (very rough, per billion parameters)
        let ram_per_billion = match device {
            DeviceType::Cpu => 1.0 * quant_factor,      // CPU: weights in RAM
            DeviceType::Cuda => 0.5 * quant_factor,     // GPU: weights in VRAM
            DeviceType::Metal => 0.7 * quant_factor,    // Unified memory
            DeviceType::Vulkan => 0.6 * quant_factor,
            DeviceType::Auto => 1.0 * quant_factor,
        };

        let ram_gb = params * ram_per_billion + 2.0; // +2GB overhead

        // CPU cores: more for larger models, agentic workloads benefit from parallelism
        let cpu_cores = ((params / 7.0).ceil() as u32).max(1).min(64);

        // GPU VRAM
        let gpu_vram_gb = match device {
            DeviceType::Cuda | DeviceType::Metal | DeviceType::Vulkan => Some(params * 0.5 * quant_factor + 1.0),
            _ => None,
        };

        // Disk space
        let disk_gb = params * 0.5 * quant_factor + 1.0;

        // Estimate tokens/sec (very rough, reference: 7B Q4 on modern CPU ~30 tok/s)
        let est_tokens_per_sec = Some((30.0 * (7.0 / params)) / quant_factor);

        ModelRequirements {
            cpu_cores,
            ram_gb,
            gpu_vram_gb,
            disk_gb,
            est_tokens_per_sec,
        }
    }

    /// Parse parameter count from string like "7B", "70B"
    fn parse_parameters(&self) -> Option<f32> {
        self.parameters.as_ref().and_then(|p| {
            p.trim_end_matches('B').parse().ok()
        })
    }

    /// Quantization factor (1.0 = FP16, ~0.25 = Q4)
    fn quantization_factor(&self) -> f32 {
        self.quantization.as_ref().map(|q| {
            let q = q.to_uppercase();
            if q.contains("Q4") || q.contains("4BIT") { 0.25 }
            else if q.contains("Q5") { 0.3125 }
            else if q.contains("Q6") { 0.375 }
            else if q.contains("Q8") || q.contains("8BIT") { 0.5 }
            else if q.contains("FP16") || q.contains("BF16") { 1.0 }
            else if q.contains("FP32") { 2.0 }
            else { 0.5 } // Default assumption
        }).unwrap_or(0.5)
    }

    /// Check if model supports a device
    pub fn supports_device(&self, device: DeviceType) -> bool {
        self.supported_devices.contains(&device) || self.supported_devices.contains(&DeviceType::Auto)
    }
}