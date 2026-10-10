use super::*;

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
            .expect("run should start")
            .await,
        Err(LlmError::InvalidModel(_))
    ));
}

#[tokio::test]
async fn cancelling_before_first_poll_never_invokes_the_provider() {
    let provider = ScriptedProvider::new(BuiltInProvider::Ollama, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    let run = runtime
        .request("ollama")
        .user_message("stop")
        .send()
        .expect("run should start");
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
    let mut run = runtime
        .request("claude")
        .user_message("stream")
        .send()
        .expect("run should start");
    let stop_handle = run.stop_handle();
    let mut events = run.take_event_stream().unwrap();
    let run_task = tokio::spawn(run);
    while let Some(event) = events.next().await {
        if matches!(event.event(), Some(RunEvent::ResponseDelta(_))) {
            stop_handle.stop();
            break;
        }
    }
    let LlmRunOutcome::Cancelled(partial_response) = run_task.await.unwrap().unwrap() else {
        panic!("expected cancellation");
    };
    assert_eq!(partial_response.assistant_rounds[0].text, "partial");
    assert_eq!(
        partial_response.assistant_rounds[0]
            .visible_reasoning
            .as_deref(),
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
    let outcome = runtime
        .request("gemini")
        .user_message("identity")
        .send()
        .expect("run should start")
        .await
        .unwrap();
    let LlmRunOutcome::Failed(failed_run) = outcome else {
        panic!("expected identity failure")
    };
    assert!(matches!(failed_run.error, LlmError::ProviderProtocol(_)));
    assert_eq!(
        failed_run.partial_response.assistant_rounds[0].text,
        "wrong"
    );
    assert_eq!(failed_run.partial_response.provider.as_str(), "gemini");
}

#[tokio::test]
async fn failed_run_preserves_partial_streams_usage_and_completed_tool_records() {
    let call = ToolCall {
        id: "call-1".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value": "side effect"}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::StreamAndFailAfterToolRound(call),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(common::tool::EchoTool {
            execution_count: executions.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .build()
        .unwrap();

    let outcome = runtime
        .request("gemini")
        .tools(["echo"])
        .user_message("run")
        .send()
        .unwrap()
        .await
        .unwrap();
    let LlmRunOutcome::Failed(failed_run) = outcome else {
        panic!("expected failed run with a partial response")
    };

    assert_eq!(
        failed_run.error,
        LlmError::Provider("scripted provider failure".into())
    );
    assert_eq!(
        failed_run.partial_response.assistant_rounds[0].text,
        "partial answer"
    );
    assert_eq!(
        failed_run.partial_response.assistant_rounds[0]
            .visible_reasoning
            .as_deref(),
        Some("partial reasoning")
    );
    assert_eq!(failed_run.partial_response.usage.total_tokens, Some(7));
    assert_eq!(failed_run.partial_response.tool_executions.len(), 1);
    assert_eq!(
        failed_run.partial_response.tool_executions[0].call_id,
        "call-1"
    );
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn finished_event_retry_preserves_sequence_without_repeating_tool_effect() {
    let call = ToolCall {
        id: "call-retry".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value": "once"}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::RetryToolRoundAfterFinishedFailure(call),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(common::tool::EchoTool {
            execution_count: executions.clone(),
        })
        .unwrap();
    let sink = Arc::new(FailFirstFinishedSink::default());
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .event_sink(sink.clone())
        .build()
        .unwrap();

    assert!(matches!(
        runtime
            .request("gemini")
            .tools(["echo"])
            .user_message("retry")
            .send()
            .unwrap()
            .await,
        Ok(LlmRunOutcome::Completed(_))
    ));

    let failed_sequence = sink.failed_sequence.load(Ordering::SeqCst);
    let persisted_events = sink.persisted_events.lock().unwrap();
    let finished_events = persisted_events
        .iter()
        .filter(|event| matches!(&event.event, RunEvent::Tool(ToolEvent::Finished { .. })))
        .collect::<Vec<_>>();
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(finished_events.len(), 1);
    assert_eq!(finished_events[0].sequence, failed_sequence);
}
