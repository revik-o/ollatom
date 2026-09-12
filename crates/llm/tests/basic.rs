use std::str::FromStr;

#[test]
fn openai_is_a_provider_alias_for_chatgpt() {
    assert_eq!(
        llm::ProviderId::from_str("openai").unwrap().as_str(),
        "chatgpt"
    );
}

#[test]
fn fluent_request_builder_accepts_the_complete_developer_surface() {
    let _request = llm::Llm::init("ollama")
        .model("qwen3:4b")
        .effort("medium")
        .trusted_folders(["/tmp/work"])
        .trusted_commands(["cargo test(?: .*)?"])
        .allowed(llm::ALL_FILESYSTEM_ACCESS | llm::ALL_USER_COMMANDS)
        .on_reasoning_delta(|delta| async move {
            let _: String = delta;
        })
        .on_response_delta(|delta| async move {
            let _: String = delta;
        })
        .on_usage(|usage| async move {
            let _: llm::Usage = usage;
        })
        .user_message("hello");
}

#[test]
fn generation_options_serialize_with_the_public_field_names() {
    let options = llm::GenerationOptions {
        max_output_tokens: Some(128),
        temperature: Some(0.2),
        max_token_choices: Some(40),
        diversity_threshold: Some(0.9),
        repeat_penalty: Some(1.1),
        stop_sequences: Some(vec!["END".into()]),
    };
    let serialized = serde_json::to_value(options).unwrap();
    assert_eq!(serialized["max_token_choices"], serde_json::json!(40));
    let serialized_diversity_threshold = serialized["diversity_threshold"].as_f64().unwrap();
    assert!((serialized_diversity_threshold - 0.9).abs() < 1e-6);
    assert!(serialized.get("top_p").is_none());
    assert!(serialized.get("top_k").is_none());
}
