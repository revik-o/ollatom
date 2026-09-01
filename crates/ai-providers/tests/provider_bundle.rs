use ai_providers::{
    BuiltInProvider, BuiltInProviderConfiguration, BuiltInProviders, LlmError, ModelId, ProviderId,
    chatgpt, claude, gemini, llama_cpp, lm_studio, ollama,
};

#[test]
fn provider_crates_declare_their_builtin_identifiers() {
    assert_eq!(chatgpt::BUILT_IN_PROVIDER, BuiltInProvider::ChatGpt);
    assert_eq!(claude::BUILT_IN_PROVIDER, BuiltInProvider::Claude);
    assert_eq!(gemini::BUILT_IN_PROVIDER, BuiltInProvider::Gemini);
    assert_eq!(ollama::BUILT_IN_PROVIDER, BuiltInProvider::Ollama);
    assert_eq!(llama_cpp::BUILT_IN_PROVIDER, BuiltInProvider::LlamaCpp);
    assert_eq!(lm_studio::BUILT_IN_PROVIDER, BuiltInProvider::LmStudio);
}

#[test]
fn bundle_configuration_rejects_a_default_for_an_unregistered_provider() {
    let provider_id = ProviderId::new("ollama").unwrap();
    let default_model = ModelId::new("qwen3:4b").unwrap();
    let configuration =
        BuiltInProviderConfiguration::default().default_model(provider_id, default_model);
    let build_result = BuiltInProviders::builder()
        .configuration(configuration)
        .build();
    let has_unregistered_provider_error =
        matches!(build_result, Err(LlmError::ProviderNotRegistered(_)));

    assert!(has_unregistered_provider_error);
}

#[test]
fn bundle_configuration_rejects_invalid_provider_identifiers() {
    let configuration =
        BuiltInProviderConfiguration::default().default_model("invalid provider!", "model");
    let build_result = BuiltInProviders::builder()
        .configuration(configuration)
        .build();
    let has_invalid_provider_error = matches!(build_result, Err(LlmError::InvalidProvider(_)));

    assert!(has_invalid_provider_error);
}

#[test]
fn bundle_configuration_rejects_empty_model_identifiers() {
    let configuration = BuiltInProviderConfiguration::default().default_model("ollama", "  ");
    let build_result = BuiltInProviders::builder()
        .configuration(configuration)
        .build();
    let has_invalid_model_error = matches!(build_result, Err(LlmError::InvalidModel(_)));

    assert!(has_invalid_model_error);
}
