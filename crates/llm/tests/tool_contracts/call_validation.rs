use super::*;

#[tokio::test]
async fn duplicate_tool_call_ids_reuse_the_first_result() {
    let call = ToolCall {
        id: "call-1".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value":"x"}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::ExecuteToolRound(vec![call.clone(), call]),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(EchoTool {
            execution_count: executions.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "gemini-test")
        .tools(tools)
        .build()
        .unwrap();
    let outcome = runtime
        .request("gemini")
        .tools(["echo"])
        .user_message("run")
        .send()
        .expect("run should start")
        .await
        .unwrap();
    let LlmRunOutcome::Completed(response) = outcome else {
        panic!("expected a completed response");
    };
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(response.tool_executions.len(), 1);
    assert_eq!(response.tool_executions[0].call_id, "call-1");
}

#[tokio::test]
async fn duplicate_tool_call_ids_with_changed_arguments_are_protocol_errors() {
    let first = ToolCall {
        id: "call-1".into(),
        name: "echo".into(),
        arguments: serde_json::json!({"value":"x"}),
    };
    let second = ToolCall {
        arguments: serde_json::json!({"value":"y"}),
        ..first.clone()
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::ExecuteToolRound(vec![first, second]),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(EchoTool {
            execution_count: executions,
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "gemini-test")
        .tools(tools)
        .build()
        .unwrap();
    assert!(matches!(
        runtime
            .request("gemini")
            .tools(["echo"])
            .user_message("run")
            .send()
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::ToolProtocol(_),
            ..
        }))
    ));
}

#[tokio::test]
async fn duplicate_tool_call_ids_cannot_switch_tool_names() {
    let first = ToolCall {
        id: "call-1".into(),
        name: "first".into(),
        arguments: serde_json::json!({"value":"x"}),
    };
    let second = ToolCall {
        name: "second".into(),
        ..first.clone()
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::ExecuteToolRound(vec![first, second]),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    for name in ["first", "second"] {
        tools
            .register(NamedEchoTool {
                name,
                execution_count: executions.clone(),
            })
            .unwrap();
    }
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .build()
        .unwrap();
    assert!(matches!(
        runtime
            .request("gemini")
            .tools(["first", "second"])
            .user_message("run")
            .send()
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::ToolProtocol(_),
            ..
        }))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn empty_tool_call_identifier_is_rejected_before_execution() {
    let provider = ScriptedProvider::new(
        BuiltInProvider::Gemini,
        ProviderBehavior::ExecuteToolRound(vec![ToolCall {
            id: " \t".into(),
            name: "echo".into(),
            arguments: serde_json::json!({"value": "x"}),
        }]),
    );
    let executions = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    tools
        .register(EchoTool {
            execution_count: executions.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "gemini-test")
        .tools(tools)
        .build()
        .unwrap();

    assert!(matches!(
        runtime
            .request("gemini")
            .tools(["echo"])
            .user_message("run")
            .send()
            .expect("run should start")
            .await,
        Ok(LlmRunOutcome::Failed(FailedRun {
            error: LlmError::ToolProtocol(message),
            ..
        })) if message.contains("ID must be non-empty")
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_tool_arguments_are_returned_to_the_model() {
    let call = ToolCall {
        id: "invalid".into(),
        name: "echo".into(),
        arguments: serde_json::json!({}),
    };
    let provider = ScriptedProvider::new(
        BuiltInProvider::Ollama,
        ProviderBehavior::ExecuteToolRound(vec![call]),
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
    let LlmRunOutcome::Completed(response) = runtime
        .request("ollama")
        .tools(["echo"])
        .user_message("run")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected model-visible failure");
    };
    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::InvalidArguments(_))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}
