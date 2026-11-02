use polygone_petals::backends::ollama::OllamaBackend;
use polygone_petals::types::{ChatMessage, ChatRequest, GenerationConfig, InferenceRequest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🌸 Testing Petals + Ollama integration...");

    // Create Ollama backend
    let backend = OllamaBackend::new().await?;
    println!("✅ OllamaBackend created");

    // Test list models
    let models = backend.list_models().await?;
    println!("📋 Models available: {}", models.len());
    for m in &models {
        let size = m
            .size_gb
            .map(|s| format!("{:.1} GB", s))
            .unwrap_or("unknown".to_string());
        println!("  - {} ({})", m.name, size);
    }

    // Test inference
    let request = InferenceRequest::new("Say hello in one sentence.")
        .with_model("phi4-mini:latest")
        .with_config(GenerationConfig {
            temperature: Some(0.7),
            max_tokens: Some(50),
            ..Default::default()
        });

    println!("\n🤖 Running inference...");
    let response = backend.generate(request).await?;
    println!("✅ Response: {}", response.text);
    println!(
        "   Tokens: {}, Time: {}ms, Tokens/s: {:.1}",
        response.tokens_generated, response.total_time_ms, response.tokens_per_second
    );

    // Test chat
    let chat_request = ChatRequest {
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "What is 2+2?".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }],
        model: Some("phi4-mini:latest".to_string()),
        config: GenerationConfig {
            temperature: Some(0.1),
            max_tokens: Some(20),
            ..Default::default()
        },
        stream: false,
        tools: None,
        tool_choice: None,
    };

    println!("\n💬 Testing chat...");
    let chat_response = backend.chat(chat_request).await?;
    println!("✅ Chat response: {}", chat_response.message.content);

    println!("\n🎉 All tests passed!");
    Ok(())
}
