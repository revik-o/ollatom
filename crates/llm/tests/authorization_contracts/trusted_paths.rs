use super::*;

#[tokio::test]
async fn explicit_denials_are_returned_to_the_model() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_protected_tool(
        BuiltInProvider::Claude,
        protected_tool_call("write-2", "/tmp/work/file"),
        execution_count.clone(),
    );

    let LlmRunOutcome::Completed(response) = runtime
        .request("claude")
        .tools(["write_file"])
        .deny_untrusted()
        .user_message("write")
        .send()
        .expect("run should start")
        .await
        .unwrap()
    else {
        panic!("expected provider recovery from a denied tool result");
    };

    assert!(matches!(
        response.tool_executions[0].output.as_failure(),
        Some(ToolFailure::Denied(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn trusted_folders_authorize_paths_beneath_the_trusted_root() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_protected_tool(
        BuiltInProvider::Gemini,
        protected_tool_call("write-3", "/tmp/work/file"),
        execution_count.clone(),
    );

    let LlmRunOutcome::Completed(response) = runtime
        .request("gemini")
        .tools(["write_file"])
        .trusted_folders(["/tmp/work"])
        .deny_untrusted()
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
}

#[tokio::test]
async fn approval_callbacks_can_authorize_paths_outside_trusted_roots() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_protected_tool(
        BuiltInProvider::ChatGpt,
        protected_tool_call("write-4", "/outside/file"),
        execution_count.clone(),
    );

    runtime
        .request("chatgpt")
        .tools(["write_file"])
        .trusted_folders(["/tmp/work"])
        .on_approval_request(|approval_request| async move {
            assert_eq!(approval_request.tool_name, "write_file");
            ApprovalDecision::AllowOnce
        })
        .user_message("write")
        .send()
        .expect("run should start")
        .await
        .unwrap();

    assert_eq!(execution_count.load(Ordering::SeqCst), 1);
}

#[test]
fn command_patterns_are_anchored() {
    let command_pattern = CommandPattern::new("cargo test").unwrap();
    assert!(command_pattern.matches("cargo test"));
    assert!(command_pattern.matches_command("cargo", &["test".into()]));
    assert!(!command_pattern.matches("prefix cargo test"));
}

#[test]
fn trusted_folder_deletion_requires_an_explicit_full_access_grant() {
    let standard_access = TrustedFolder::standard("/tmp/work");
    let full_access = TrustedFolder::full_access("/tmp/work");
    assert!(!standard_access.allow_delete);
    assert!(full_access.allow_delete);
}
