use super::*;

#[tokio::test]
async fn conversation_tool_results_must_consume_unique_historical_calls() {
    let provider = ScriptedProvider::new(BuiltInProvider::Ollama, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    let tool_call = ToolCall {
        id: "call-1".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value": "one"}),
    };
    let assistant_call = ConversationMessage {
        role: ConversationRole::Assistant,
        content: vec![ContentBlock::ToolCall {
            call: tool_call.clone(),
        }],
    };
    let tool_result = ConversationMessage {
        role: ConversationRole::Tool,
        content: vec![ContentBlock::ToolResult {
            output: ToolOutput::success("call-1", serde_json::json!("one")),
        }],
    };

    let replayed_result = runtime
        .request(BuiltInProvider::Ollama)
        .context(vec![assistant_call.clone(), tool_result.clone(), tool_result.clone()])
        .user_message("continue")
        .send()
        .unwrap()
        .await;

    assert!(matches!(
        replayed_result,
        Err(LlmError::InvalidRequest(message)) if message.contains("without an unmatched call")
    ));

    let reused_call_identifier = runtime
        .request(BuiltInProvider::Ollama)
        .context(vec![
            assistant_call.clone(),
            tool_result.clone(),
            assistant_call,
            tool_result,
        ])
        .user_message("continue")
        .send()
        .unwrap()
        .await;

    assert!(matches!(
        reused_call_identifier,
        Err(LlmError::InvalidRequest(message)) if message.contains("duplicate tool call identifier")
    ));
    assert!(provider.recorded_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn empty_user_messages_and_history_entries_are_rejected() {
    let provider = ScriptedProvider::new(BuiltInProvider::Ollama, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();

    let empty_user_message = runtime
        .request(BuiltInProvider::Ollama)
        .user_message(UserMessage::new([]))
        .send()
        .unwrap()
        .await;
    assert!(matches!(empty_user_message, Err(LlmError::InvalidRequest(_))));

    let empty_history_message = runtime
        .request(BuiltInProvider::Ollama)
        .context(vec![ConversationMessage {
            role: ConversationRole::User,
            content: Vec::new(),
        }])
        .user_message("continue")
        .send()
        .unwrap()
        .await;
    assert!(matches!(empty_history_message, Err(LlmError::InvalidRequest(_))));
    assert!(provider.recorded_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn durable_sink_failure_closes_the_event_stream() {
    let provider = ScriptedProvider::new(BuiltInProvider::ChatGpt, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .event_sink(Arc::new(RejectingSink))
        .build()
        .unwrap();
    let mut run = runtime
        .request("chatgpt")
        .user_message("journal")
        .send()
        .expect("run should start");
    let mut events = run.take_event_stream().unwrap();
    assert!(matches!(
        run.await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::EventSink(_),
            terminal_event_delivered: false,
            ..
        }))
    ));
    let next_event = tokio::time::timeout(std::time::Duration::from_secs(1), events.next())
        .await
        .unwrap();
    assert!(matches!(next_event, Some(RunStreamItem::DeliveryFailed(_))));
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn event_sink_wait_is_bounded_by_the_configured_delivery_timeout() {
    let provider = ScriptedProvider::new(BuiltInProvider::ChatGpt, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .event_sink(Arc::new(BlockingEventSink))
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.transport.event_delivery_timeout_ms = Some(5);

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime
            .request("chatgpt")
            .options(options)
            .user_message("bounded sink")
            .send()
            .expect("run should start"),
    )
    .await
    .expect("event sink timeout should release the run")
    .unwrap();

    assert!(matches!(
        outcome,
        LlmRunOutcome::Failed(FailedRun {
            error: LlmError::EventSink(_),
            terminal_event_delivered: false,
            ..
        })
    ));
}

#[tokio::test]
async fn zero_event_delivery_timeout_is_rejected_before_provider_startup() {
    let provider = ScriptedProvider::new(BuiltInProvider::ChatGpt, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider.clone(), "model")
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.transport.event_delivery_timeout_ms = Some(0);

    assert!(matches!(
        runtime
            .request("chatgpt")
            .options(options)
            .user_message("invalid timeout")
            .send()
            .expect("run should start")
            .await,
        Err(LlmError::InvalidRequest(message)) if message.contains("timeouts must be greater than zero")
    ));
    assert!(provider.recorded_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn overall_deadline_bounds_terminal_event_persistence() {
    let provider = ScriptedProvider::new(BuiltInProvider::ChatGpt, ProviderBehavior::Complete);
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .event_sink(Arc::new(BlockingCompletionSink))
        .build()
        .unwrap();
    let mut options = LlmOptions::default();
    options.transport.overall_timeout_ms = Some(50);

    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime
            .request("chatgpt")
            .options(options)
            .user_message("bounded terminal event")
            .send()
            .expect("run should start"),
    )
    .await
    .expect("overall deadline should release terminal delivery")
    .unwrap();
    let LlmRunOutcome::Failed(failed_run) = outcome else {
        panic!("terminal delivery timeout should preserve a failed outcome")
    };

    assert!(matches!(failed_run.error, LlmError::Timeout(_)));
    assert_eq!(failed_run.partial_response.assistant_rounds[0].text, "ok");
    assert!(!failed_run.terminal_event_delivered);
}
