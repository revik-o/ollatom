#[allow(dead_code)]
mod support;

use llm::{CapabilitySupport, LlmProvider, ModelCapability};
use ollama::OllamaProvider;
use serde_json::json;
use support::{ScriptedResponse, ScriptedServer};

#[tokio::test]
async fn model_information_preserves_alias_and_complete_native_metadata() {
    let scripted_server = ScriptedServer::start(vec![
        ScriptedResponse::json(
            200,
            "OK",
            json!({
                "details": {
                    "family": "gemma",
                    "parameter_size": "7.2B",
                    "quantization_level": "Q4_K_M"
                },
                "model_info": {"gemma.context_length": 131072},
                "capabilities": ["completion", "tools", "thinking"],
                "license": "test-license",
                "future_field": {"preserved": true}
            }),
        ),
        ScriptedResponse::json(
            200,
            "OK",
            json!({
                "models": [{
                    "name": "custom-alias",
                    "model": "canonical-model",
                    "digest": "test-digest",
                    "size": 1234
                }]
            }),
        ),
        ScriptedResponse::json(
            200,
            "OK",
            json!({
                "models": [{
                    "name": "custom-alias",
                    "model": "canonical-model",
                    "size_vram": 987
                }]
            }),
        ),
    ]);
    let provider = OllamaProvider::builder()
        .endpoint(scripted_server.endpoint())
        .build()
        .unwrap();
    let requested_model = llm::ModelId::new("custom-alias").unwrap();

    let model_information = provider.model_info(&requested_model).await.unwrap();

    assert_eq!(model_information.id, requested_model);
    assert_eq!(model_information.context_window.value, Some(131_072));
    assert_eq!(
        model_information.quantization.value.as_deref(),
        Some("Q4_K_M")
    );
    assert_eq!(
        model_information
            .capabilities
            .support_for(ModelCapability::Tools),
        CapabilitySupport::Supported
    );
    let native_metadata = &model_information.provider_metadata["ollama"];
    assert_eq!(native_metadata["parameter_count"], json!(7_200_000_000_u64));
    assert_eq!(native_metadata["show"]["license"], json!("test-license"));
    assert_eq!(
        native_metadata["show"]["future_field"]["preserved"],
        json!(true)
    );
    assert_eq!(native_metadata["tag"]["digest"], json!("test-digest"));
    assert_eq!(native_metadata["running"]["size_vram"], json!(987));
    scripted_server.finish();
}
