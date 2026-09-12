mod support;

use llm::{
    AvailabilityState, BuiltInProvider, CapabilitySupport, GenerationOptions, LlmOptions,
    LlmProvider, LlmRunOutcome, ModelCapability,
};
use ollama::{DEFAULT_OLLAMA_ENDPOINT, OllamaProvider};
use serde_json::json;
use std::time::Duration;
use support::{ScriptedResponse, ScriptedServer};

#[test]
fn default_configuration_uses_ollama_endpoint_and_identity() {
    let provider = OllamaProvider::new();
    assert_eq!(
        provider.endpoint().as_str(),
        format!("{DEFAULT_OLLAMA_ENDPOINT}/")
    );
    assert_eq!(
        provider.id(),
        &llm::ProviderId::from(BuiltInProvider::Ollama)
    );
}

#[test]
fn endpoint_without_scheme_is_normalized() {
    let provider = OllamaProvider::builder()
        .endpoint("localhost:11435")
        .discovery_timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    assert_eq!(provider.endpoint().as_str(), "http://localhost:11435/");
}

#[test]
fn provider_capabilities_are_conservative() {
    let provider = OllamaProvider::new();
    let capabilities = provider.capabilities();
    assert_eq!(
        capabilities.support_for(ModelCapability::Streaming),
        CapabilitySupport::Supported
    );
    assert_eq!(
        capabilities.support_for(ModelCapability::Tools),
        CapabilitySupport::Unknown
    );
    assert_eq!(
        capabilities.support_for(ModelCapability::Files),
        CapabilitySupport::Unsupported
    );
    assert_eq!(
        capabilities.generation_options.get("max_token_choices"),
        Some(&CapabilitySupport::Supported)
    );
    assert_eq!(
        capabilities.generation_options.get("diversity_threshold"),
        Some(&CapabilitySupport::Supported)
    );
    assert!(!capabilities.generation_options.contains_key("top_p"));
    assert!(!capabilities.generation_options.contains_key("top_k"));
}

#[tokio::test]
async fn unavailable_daemon_returns_structured_report_without_credentials() {
    let provider = OllamaProvider::builder()
        .endpoint("127.0.0.1:1")
        .discovery_timeout(Duration::from_millis(50))
        .bearer_token("secret-token")
        .build()
        .unwrap();
    let model = llm::ModelId::new("gemma4:e2b").unwrap();
    let report = provider.availability(Some(&model)).await.unwrap();
    assert_eq!(report.selected_model, Some(model));
    assert!(!report.message.unwrap_or_default().contains("secret-token"));
}

#[tokio::test]
async fn unauthorized_daemon_reports_a_reachable_endpoint_and_failed_authentication() {
    let scripted_server = ScriptedServer::start(vec![ScriptedResponse::json(
        401,
        "Unauthorized",
        json!({"error": "unauthorized"}),
    )]);
    let provider = OllamaProvider::builder()
        .endpoint(scripted_server.endpoint())
        .bearer_token("secret-token")
        .build()
        .unwrap();

    let report = provider.availability(None).await.unwrap();

    assert_eq!(report.state, AvailabilityState::Unauthorized);
    assert_eq!(report.endpoint, AvailabilityState::Ready);
    assert_eq!(report.authentication, AvailabilityState::Unauthorized);
    let requests = scripted_server.finish();
    assert!(requests[0].headers.contains("secret-token"));
    assert!(!format!("{provider:?}").contains("secret-token"));
}

#[tokio::test]
async fn malformed_version_reports_an_unknown_service_on_a_reachable_endpoint() {
    let scripted_server = ScriptedServer::start(vec![ScriptedResponse::json(
        200,
        "OK",
        json!({"unexpected": true}),
    )]);
    let provider = OllamaProvider::builder()
        .endpoint(scripted_server.endpoint())
        .build()
        .unwrap();

    let report = provider.availability(None).await.unwrap();

    assert_eq!(report.state, AvailabilityState::Unknown);
    assert_eq!(report.endpoint, AvailabilityState::Ready);
    assert_eq!(report.authentication, AvailabilityState::Ready);
    scripted_server.finish();
}

#[tokio::test]
async fn missing_model_report_preserves_the_exact_requested_identifier() {
    let scripted_server = ScriptedServer::start(vec![
        ScriptedResponse::json(200, "OK", json!({"version": "test"})),
        ScriptedResponse::json(404, "Not Found", json!({"error": "missing"})),
    ]);
    let provider = OllamaProvider::builder()
        .endpoint(scripted_server.endpoint())
        .build()
        .unwrap();
    let requested_model = llm::ModelId::new("custom-alias").unwrap();

    let report = provider.availability(Some(&requested_model)).await.unwrap();

    assert_eq!(report.state, AvailabilityState::ModelMissing);
    assert_eq!(report.model, AvailabilityState::ModelMissing);
    assert_eq!(report.selected_model, Some(requested_model));
    let requests = scripted_server.finish();
    assert_eq!(requests[1].request_line, "POST /api/show HTTP/1.1");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&requests[1].body).unwrap()["name"],
        json!("custom-alias")
    );
}

#[tokio::test]
async fn renamed_generation_options_map_to_the_native_ollama_fields() {
    let scripted_server = ScriptedServer::start(vec![
        ScriptedResponse::json(200, "OK", json!({"capabilities": []})),
        ScriptedResponse::stream(
            json!({
                "message": {"role": "assistant", "content": "done"},
                "done": true,
                "done_reason": "stop"
            })
            .to_string(),
        ),
    ]);
    let provider = OllamaProvider::builder()
        .endpoint(scripted_server.endpoint())
        .build()
        .unwrap();
    let runtime = llm::LlmRuntime::builder()
        .provider_with_default(std::sync::Arc::new(provider), "test-model")
        .build()
        .unwrap();
    let options = LlmOptions {
        generation: GenerationOptions {
            max_token_choices: Some(41),
            diversity_threshold: Some(0.73),
            ..GenerationOptions::default()
        },
        ..LlmOptions::default()
    };

    let outcome = runtime
        .request(BuiltInProvider::Ollama)
        .options(options)
        .user_message("hello")
        .send()
        .await
        .unwrap();

    assert!(matches!(outcome, LlmRunOutcome::Completed(_)));
    let requests = scripted_server.finish();
    assert_eq!(requests[1].request_line, "POST /api/chat HTTP/1.1");
    let request_body = serde_json::from_str::<serde_json::Value>(&requests[1].body).unwrap();
    assert_eq!(request_body["options"]["top_k"], json!(41));
    let native_diversity_threshold = request_body["options"]["top_p"].as_f64().unwrap();
    assert!((native_diversity_threshold - 0.73).abs() < f64::EPSILON * 1_000_000_000.0);
    assert!(request_body["options"].get("max_token_choices").is_none());
    assert!(request_body["options"].get("diversity_threshold").is_none());
}
