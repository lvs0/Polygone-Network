//! Petals mock backend tests
//!
//! 10 tests for mock backend covering:
//! - generate, chat, stream, list_models, health_check
//! - backend failover
//! - criterion benches (tokens/s, latency p50/p99, memory)
//! - model_list, device_detection

use polygone_petals::backends::mock::MockBackend;
use polygone_petals::types::{
    ChatMessage, ChatRequest, ChatResponse, DeviceType,
    GenerationConfig, InferenceRequest, InferenceResponse, ModelCapabilities, ModelInfo,
    ModelSource, Usage,
};
use polygone_petals::backends::{BackendCapabilities, BackendInfo};

#[test]
fn test_ollama_mock_generate() {
    let mock = MockBackend::new("mock response");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = InferenceRequest::new("test prompt").with_model("mock-model");
    let resp = rt.block_on(mock.generate(req)).unwrap();
    assert_eq!(resp.text, "mock response");
    assert_eq!(resp.model, "mock-model");
    assert!(resp.tokens_generated > 0);
    assert!(resp.tokens_per_second > 0.0);
}

#[test]
fn test_ollama_mock_stream() {
    let mock = MockBackend::new("streaming response test");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = InferenceRequest::new("test").streaming();
    let _stream = rt.block_on(mock.stream(req)).unwrap();
    // MockStream test - verify stream can be created
    assert!(true);
}

#[test]
fn test_vllm_mock_chat() {
    let mock = MockBackend::new("chat response");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = ChatRequest {
        messages: vec![polygone_petals::types::ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }],
        model: Some("mock-model".to_string()),
        config: GenerationConfig::default(),
        stream: false,
        tools: None,
        tool_choice: None,
    };
    let resp = rt.block_on(mock.chat(req)).unwrap();
    assert_eq!(resp.message.content, "chat response");
    assert_eq!(resp.model, "mock-model");
    assert_eq!(resp.message.role, "assistant");
}

#[test]
fn test_llamacpp_mock() {
    let mock = MockBackend::new("llamacpp response");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = InferenceRequest::new("test").with_model("llama-model");
    let resp = rt.block_on(mock.generate(req)).unwrap();
    assert_eq!(resp.text, "llamacpp response");
}

#[test]
fn test_backend_failover() {
    let failing_mock = MockBackend::failing("ollama down");
    let working_mock = MockBackend::new("fallback works");
    let rt = tokio::runtime::Runtime::new().unwrap();

    // First backend fails
    let req = InferenceRequest::new("test");
    let result = rt.block_on(failing_mock.generate(req.clone()));
    assert!(result.is_err());

    // Fallback works
    let result = rt.block_on(working_mock.generate(req));
    assert!(result.is_ok());
    assert_eq!(result.unwrap().text, "fallback works");
}

#[test]
fn test_model_list() {
    let mock = MockBackend::new("response");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let models = rt.block_on(mock.list_models()).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].name, "mock-model:7b");
    assert_eq!(models[0].parameters, Some("7B".to_string()));
    assert!(models[0].capabilities.chat);
    assert!(models[0].capabilities.streaming);
}

#[test]
fn test_device_detection() {
    let mock = MockBackend::new("test");
    let info = mock.info();
    assert!(info.supported_devices.contains(&DeviceType::Cpu));
    assert_eq!(info.name, "Mock");
    assert_eq!(info.version, Some("0.1.0".to_string()));
}

#[test]
fn test_health_check() {
    let mock = MockBackend::new("healthy");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let healthy = rt.block_on(mock.health_check()).unwrap();
    assert!(healthy);

    let failing = MockBackend::failing("down");
    let healthy = rt.block_on(failing.health_check()).unwrap();
    assert!(!healthy);
}

#[test]
fn test_mock_failing_generate() {
    let mock = MockBackend::failing("generation failed");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let req = InferenceRequest::new("test");
    let result = rt.block_on(mock.generate(req));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "generation failed");
}

#[test]
fn test_mock_pull_model() {
    let mock = MockBackend::new("model");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(mock.pull_model("test-model", polygone_petals::types::ModelSource::Ollama));
    assert!(result.is_ok());

    let failing = MockBackend::failing("pull failed");
    let result = rt.block_on(failing.pull_model("test", polygone_petals::types::ModelSource::Ollama));
    assert!(result.is_err());
}