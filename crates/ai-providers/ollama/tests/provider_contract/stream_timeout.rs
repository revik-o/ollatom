use super::*;

fn frame(content: &str, done: bool) -> String {
    format!(
        "{}\n",
        json!({
            "message": {"role": "assistant", "content": content},
            "done": done,
            "done_reason": if done { Some("stop") } else { None },
        })
    )
}

async fn run_response(response: ScriptedResponse, idle_timeout_ms: Option<u64>) -> LlmRunOutcome {
    let server = ScriptedServer::start(vec![
        ScriptedResponse::json(200, "OK", json!({"capabilities": ["completion"]})),
        response,
    ]);
    let provider = OllamaProvider::builder().endpoint(server.endpoint()).build().unwrap();
    let runtime = llm::LlmRuntime::builder()
        .provider_with_default(std::sync::Arc::new(provider), "test-model")
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.transport.stream_idle_timeout_ms = idle_timeout_ms;
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        runtime
            .request(BuiltInProvider::Ollama)
            .options(options)
            .user_message("hello")
            .send()
            .unwrap(),
    )
    .await
    .expect("test run must finish")
    .unwrap();
    server.finish();
    outcome
}

fn require_idle_timeout(outcome: LlmRunOutcome) {
    let LlmRunOutcome::Failed(failed) = outcome else {
        panic!("expected idle timeout")
    };
    assert!(matches!(failed.error, llm::LlmError::Timeout(message) if message.contains("stream idle timeout")));
}

#[tokio::test]
async fn idle_timeout_bounds_wait_for_response_headers() {
    require_idle_timeout(
        run_response(
            ScriptedResponse::stream(frame("done", true)).delayed_headers(Duration::from_millis(300)),
            Some(100),
        )
        .await,
    );
}

#[tokio::test]
async fn idle_timeout_bounds_wait_for_first_body_chunk() {
    require_idle_timeout(
        run_response(
            ScriptedResponse::delayed_stream(vec![(Duration::from_millis(300), frame("done", true))]),
            Some(100),
        )
        .await,
    );
}

#[tokio::test]
async fn idle_timeout_resets_on_chunks_and_preserves_partial_output_on_stall() {
    let outcome = run_response(
        ScriptedResponse::delayed_stream(vec![
            (Duration::ZERO, frame("partial", false)),
            (Duration::from_millis(300), frame("done", true)),
        ]),
        Some(100),
    )
    .await;
    let LlmRunOutcome::Failed(failed) = &outcome else {
        panic!("expected idle timeout")
    };
    assert_eq!(failed.partial_response.assistant_rounds[0].text, "partial");
    require_idle_timeout(outcome);
}

#[tokio::test]
async fn active_stream_can_exceed_one_idle_timeout_period() {
    let outcome = run_response(
        ScriptedResponse::delayed_stream(vec![
            (Duration::ZERO, frame("one", false)),
            (Duration::from_millis(150), frame("two", false)),
            (Duration::from_millis(150), frame("three", false)),
            (Duration::from_millis(150), frame("done", true)),
        ]),
        Some(300),
    )
    .await;
    let LlmRunOutcome::Completed(response) = outcome else {
        panic!("active stream should complete")
    };
    assert_eq!(response.assistant_rounds[0].text, "onetwothreedone");
}

#[tokio::test]
async fn idle_timeout_is_optional() {
    let outcome = run_response(
        ScriptedResponse::stream(frame("done", true)).delayed_headers(Duration::from_millis(300)),
        None,
    )
    .await;
    assert!(matches!(outcome, LlmRunOutcome::Completed(_)));
}
