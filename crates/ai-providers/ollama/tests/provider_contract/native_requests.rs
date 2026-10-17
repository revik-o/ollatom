use super::*;

#[tokio::test]
async fn image_and_audio_payloads_use_the_native_images_field() {
    let scripted_server = ScriptedServer::start(vec![
        ScriptedResponse::json(200, "OK", json!({"capabilities": ["completion", "vision", "audio"]})),
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
    let user_message = UserMessage::new([
        ContentBlock::Binary {
            media_type: "image/png".into(),
            filename: Some("image.png".into()),
            data: vec![1, 2, 3],
        },
        ContentBlock::Binary {
            media_type: "audio/wav".into(),
            filename: Some("audio.wav".into()),
            data: vec![4, 5, 6],
        },
    ]);

    let outcome = runtime
        .request(BuiltInProvider::Ollama)
        .user_message(user_message)
        .send()
        .unwrap()
        .await
        .unwrap();

    assert!(matches!(outcome, LlmRunOutcome::Completed(_)));
    let requests = scripted_server.finish();
    let request_body = serde_json::from_str::<serde_json::Value>(&requests[1].body).unwrap();
    assert_eq!(request_body["messages"][0]["images"].as_array().unwrap().len(), 2);
    assert!(request_body["messages"][0].get("audio").is_none());
}
