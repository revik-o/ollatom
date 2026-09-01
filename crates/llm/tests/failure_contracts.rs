mod common;
use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::Arc;

#[tokio::test]
async fn fluent_string_inputs_reject_invalid_provider_and_model_identifiers() {
    let runtime = LlmRuntime::builder().build().unwrap();
    assert!(matches!(
        runtime.request("invalid provider!").list_models().await,
        Err(LlmError::InvalidProvider(_))
    ));
    assert!(matches!(
        runtime
            .request("ollama")
            .model("  ")
            .user_message("hello")
            .send()
            .await,
        Err(LlmError::InvalidModel(_))
    ));
}

struct RejectingSink;
impl RunEventSink for RejectingSink {
    fn emit(&self, _event: SequencedEvent) -> BoxFuture<'_, Result<(), LlmError>> {
        Box::pin(async { Err(LlmError::EventSink("journal unavailable".into())) })
    }
}

#[tokio::test]
async fn cancelling_before_first_poll_never_invokes_the_provider() {
    let provider = ScriptedProvider::new(BuiltInProvider::Ollama, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    let run = runtime.request("ollama").user_message("stop").send();
    run.stop_handle().stop();
    assert!(matches!(run.await.unwrap(), LlmRunOutcome::Cancelled(_)));
    assert!(provider.recorded_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cancellation_preserves_streamed_partial_output_and_usage() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::Claude,
        ProviderBehavior::StreamUntilCancellation,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let mut run = runtime.request("claude").user_message("stream").send();
    let stop_handle = run.stop_handle();
    let mut events = run.take_event_stream().unwrap();
    let run_task = tokio::spawn(run);
    while let Some(event) = events.next().await {
        if matches!(event.event, RunEvent::ResponseDelta(_)) {
            stop_handle.stop();
            break;
        }
    }
    let LlmRunOutcome::Cancelled(partial_response) = run_task.await.unwrap().unwrap() else {
        panic!("expected cancellation");
    };
    assert_eq!(partial_response.text, "partial");
    assert_eq!(
        partial_response.visible_reasoning.as_deref(),
        Some("summary")
    );
    assert_eq!(partial_response.usage.total_tokens, Some(3));
    assert_eq!(partial_response.usage.source, UsageSource::ApiReported);
}

#[tokio::test]
async fn provider_response_identity_is_validated() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::ReturnWrongIdentity,
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    assert!(matches!(
        runtime
            .request("gemini")
            .user_message("identity")
            .send()
            .await,
        Err(LlmError::ProviderProtocol(_))
    ));
}

#[tokio::test]
async fn durable_sink_failure_closes_the_event_stream() {
    let provider = ScriptedProvider::new(BuiltInProvider::ChatGpt, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .event_sink(Arc::new(RejectingSink))
        .build()
        .unwrap();
    let mut run = runtime.request("chatgpt").user_message("journal").send();
    let mut events = run.take_event_stream().unwrap();
    assert!(matches!(run.await, Err(LlmError::EventSink(_))));
    let next_event = tokio::time::timeout(std::time::Duration::from_secs(1), events.next())
        .await
        .unwrap();
    let event_stream_is_closed = next_event.is_none();
    assert!(event_stream_is_closed);
}

#[tokio::test]
async fn provider_round_limits_are_enforced() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::LlamaCpp,
        ProviderBehavior::BeginProviderRounds(3),
    );
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .build()
        .unwrap();
    let limits = RunLimits {
        provider_rounds: 2,
        ..Default::default()
    };
    assert!(matches!(
        runtime
            .request("llama-cpp")
            .limits(limits)
            .user_message("loop")
            .send()
            .await,
        Err(LlmError::LoopLimit(_))
    ));
}
