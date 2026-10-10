use super::*;

#[tokio::test]
async fn capability_policy_tools_are_denied_without_a_granting_policy() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_protected_tool(
        BuiltInProvider::Ollama,
        protected_tool_call("write-1", "/tmp/work/file"),
        execution_count.clone(),
    );

    let LlmRunOutcome::Completed(response) = runtime
        .request("ollama")
        .tools(["write_file"])
        .user_message("write")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::Denied(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn explicit_approval_tools_need_a_configured_approval_path() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_unscoped_tool(execution_count.clone());

    assert!(matches!(
        runtime
            .request("claude")
            .tools(["unscoped_action"])
            .user_message("perform the action")
            .send()
            .expect("run should start")
            .await,
        Err(LlmError::InvalidRequest(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn explicit_approval_tools_are_not_auto_granted_without_capabilities() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_unscoped_tool(execution_count.clone());

    let LlmRunOutcome::Completed(response) = runtime
        .request("claude")
        .tools(["unscoped_action"])
        .on_approval_request(|_| async { ApprovalDecision::Deny })
        .user_message("perform the action")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::Denied(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn authorization_required_tools_can_be_approved_without_capabilities() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_unscoped_tool(execution_count.clone());

    let LlmRunOutcome::Completed(response) = runtime
        .request("claude")
        .tools(["unscoped_action"])
        .on_approval_request(|request| async move {
            assert_eq!(request.tool_name, "unscoped_action");
            assert!(request.capabilities.is_empty());
            ApprovalDecision::AllowOnce
        })
        .user_message("perform the action")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(response.tool_executions[0].output.is_success());
    assert_eq!(execution_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn broad_permissions_do_not_bypass_explicit_user_approval() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_unscoped_tool(execution_count.clone());

    let LlmRunOutcome::Completed(response) = runtime
        .request("claude")
        .tools(["unscoped_action"])
        .allowed(ALL_FILESYSTEM_ACCESS | ALL_USER_COMMANDS)
        .on_approval_request(|_| async { ApprovalDecision::Deny })
        .user_message("perform the action")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::Denied(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn runtime_authorizer_is_a_preflight_path_and_receives_full_approval_payload() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = ScriptedProvider::new(
        BuiltInProvider::Ollama,
        ProviderBehavior::ExecuteToolRound(vec![protected_tool_call(
            "write-authorized",
            "/outside/file",
        )]),
    );
    let mut tools = ToolRegistry::new();
    tools
        .register(ProtectedTool {
            execution_count: execution_count.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .tool_authorizer(Arc::new(ApprovingToolAuthorizer {
            requests: requests.clone(),
        }))
        .build()
        .unwrap();

    let LlmRunOutcome::Completed(response) = runtime
        .request("ollama")
        .tools(["write_file"])
        .user_message("write")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(response.tool_executions[0].output.is_success());
    assert_eq!(execution_count.load(Ordering::SeqCst), 1);
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].capabilities.len(), 1);
    assert_eq!(
        requests[0].normalized_arguments,
        serde_json::json!({"path": "/outside/file"})
    );
}

#[tokio::test]
async fn authorization_grants_reject_capabilities_outside_the_approved_plan() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let provider = ScriptedProvider::new(
        BuiltInProvider::Ollama,
        ProviderBehavior::ExecuteToolRound(vec![ToolCall {
            id: "claim-1".into(),
            name: "unapproved_claim".into(),
            arguments: serde_json::json!({}),
        }]),
    );
    let mut tools = ToolRegistry::new();
    tools
        .register(UnapprovedCapabilityClaimTool {
            execution_count: execution_count.clone(),
        })
        .unwrap();
    let runtime = LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .build()
        .unwrap();

    let LlmRunOutcome::Completed(response) = runtime
        .request("ollama")
        .tools(["unapproved_claim"])
        .user_message("run")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::Denied(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}
