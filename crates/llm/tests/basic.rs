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
