use super::*;

#[test]
fn tool_output_serializes_its_exclusive_result_variant() {
    let success = ToolOutput::success("success-1", serde_json::json!({"value": 1}));
    assert!(matches!(
        &success,
        ToolOutput::Success { call_id, content }
            if call_id == "success-1" && content == &serde_json::json!({"value": 1})
    ));
    assert_eq!(
        serde_json::to_value(&success).unwrap(),
        serde_json::json!({
            "outcome": "success",
            "call_id": "success-1",
            "content": {"value": 1}
        })
    );
    assert_eq!(
        serde_json::from_value::<ToolOutput>(serde_json::to_value(&success).unwrap()).unwrap(),
        success
    );

    let failure = ToolOutput::failure("failure-1", ToolFailure::Execution("operation failed".into()));
    assert!(matches!(
        &failure,
        ToolOutput::Failure {
            call_id,
            error: ToolFailure::Execution(message),
        } if call_id == "failure-1" && message == "operation failed"
    ));
    assert_eq!(
        serde_json::to_value(&failure).unwrap(),
        serde_json::json!({
            "outcome": "failure",
            "call_id": "failure-1",
            "error": {"kind": "execution", "message": "operation failed"},
        })
    );
    assert_eq!(
        serde_json::from_value::<ToolOutput>(serde_json::to_value(&failure).unwrap()).unwrap(),
        failure
    );
}

#[test]
fn tool_output_rejects_failure_variant_with_success_content() {
    let malformed = serde_json::json!({
        "outcome": "failure",
        "call_id": "mixed-1",
        "content": "success-shaped content",
        "error": {"kind": "execution", "message": "failed"}
    });

    assert!(serde_json::from_value::<ToolOutput>(malformed).is_err());
}

#[test]
fn shared_tool_round_history_orders_outputs_and_rejects_invalid_batches() {
    let first_call = ToolCall {
        id: "first".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value": 1}),
    };
    let second_call = ToolCall {
        id: "second".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value": 2}),
    };
    let mut history = Vec::new();

    append_tool_round_to_history(
        &mut history,
        "response".into(),
        Some("reasoning".into()),
        vec![first_call.clone(), second_call.clone()],
        vec![
            ToolOutput::success("second", serde_json::json!(2)),
            ToolOutput::success("first", serde_json::json!(1)),
        ],
    )
    .unwrap();

    assert_eq!(history.len(), 3);
    assert_eq!(history[0].role, ConversationRole::Assistant);
    assert!(matches!(
        &history[0].content[2],
        ContentBlock::ToolCall { call } if call == &first_call
    ));
    assert!(matches!(
        &history[1].content[0],
        ContentBlock::ToolResult { output } if output.call_id() == "first"
    ));
    assert!(matches!(
        &history[2].content[0],
        ContentBlock::ToolResult { output } if output.call_id() == "second"
    ));

    let mut invalid_history = Vec::new();
    assert!(
        append_tool_round_to_history(
            &mut invalid_history,
            "response".into(),
            None,
            vec![first_call.clone(), second_call.clone()],
            vec![
                ToolOutput::success("first", serde_json::Value::Null),
                ToolOutput::success("first", serde_json::Value::Null),
            ],
        )
        .is_err()
    );
    assert!(invalid_history.is_empty());

    assert!(
        append_tool_round_to_history(
            &mut invalid_history,
            "response".into(),
            None,
            vec![first_call.clone(), second_call.clone()],
            vec![
                ToolOutput::success("first", serde_json::Value::Null),
                ToolOutput::success("second", serde_json::Value::Null),
                ToolOutput::success("unexpected", serde_json::Value::Null),
            ],
        )
        .is_err()
    );
    assert!(invalid_history.is_empty());

    assert!(
        append_tool_round_to_history(
            &mut invalid_history,
            "response".into(),
            None,
            vec![first_call, second_call],
            vec![ToolOutput::success("first", serde_json::Value::Null)],
        )
        .is_err()
    );
    assert!(invalid_history.is_empty());
}

#[tokio::test]
async fn a_tool_batch_is_fully_limit_checked_before_side_effects() {
    let calls = ["first", "second"].map(|id| ToolCall {
        id: id.into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value":id}),
    });
    let provider = ScriptedProvider::new(
        BuiltInProvider::Claude,
        ProviderBehavior::ExecuteToolRound(calls.into()),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(EchoTool {
            execution_count: executions.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .build()
        .unwrap();
    let limits = RunLimits {
        tool_calls_per_round: 1,
        ..Default::default()
    };
    assert!(matches!(
        runtime
            .request("claude")
            .tools(["echo"])
            .limits(limits)
            .user_message("run")
            .send()
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::LoopLimit(_),
            ..
        }))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}
