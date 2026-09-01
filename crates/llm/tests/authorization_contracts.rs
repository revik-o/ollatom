mod common;

use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct ProtectedTool {
    execution_count: Arc<AtomicUsize>,
}

impl Tool for ProtectedTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".into(),
            description: "write".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["path"],
                "properties": {"path": {"type": "string"}}
            }),
            requires_authorization: true,
        }
    }

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        let requested_path = call
            .arguments
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ToolFailure::InvalidArguments("path must be a string".into()))?;

        Ok(ToolPlan {
            call: call.clone(),
            normalized_arguments: call.arguments.clone(),
            required_capabilities: vec![RequiredCapability::Filesystem {
                path: requested_path.into(),
                access: FilesystemAccess::Modify,
            }],
            timeout: None,
        })
    }

    fn execute(
        &self,
        plan: ToolPlan,
        _authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(ToolOutput::success(
                plan.call.id,
                serde_json::json!("written"),
            ))
        })
    }
}

fn protected_tool_call(call_id: &str, path: &str) -> ToolCall {
    ToolCall {
        id: call_id.into(),
        name: "write_file".into(),
        arguments: serde_json::json!({"path": path}),
    }
}

fn runtime_with_protected_tool(
    provider_id: BuiltInProvider,
    tool_call: ToolCall,
    execution_count: Arc<AtomicUsize>,
) -> LlmRuntime {
    let provider = ScriptedProvider::new(
        provider_id,
        ProviderBehavior::ExecuteToolRound(vec![tool_call]),
    );
    let mut tool_registry = ToolRegistry::new();
    tool_registry
        .register(ProtectedTool { execution_count })
        .unwrap();

    LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tool_registry)
        .build()
        .unwrap()
}

#[tokio::test]
async fn privileged_tools_require_an_explicit_policy() {
    let execution_count = Arc::new(AtomicUsize::new(0));
    let runtime = runtime_with_protected_tool(
        BuiltInProvider::Ollama,
        protected_tool_call("write-1", "/tmp/work/file"),
        execution_count.clone(),
    );

    assert!(matches!(
        runtime
            .request("ollama")
            .tools(["write_file"])
            .user_message("write")
            .send()
            .await,
        Err(LlmError::InvalidRequest(_))
    ));
    assert_eq!(execution_count.load(Ordering::SeqCst), 0);
}

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
        .await
        .unwrap()
    else {
        panic!("expected provider recovery from a denied tool result");
    };

    assert!(matches!(
        response.tool_executions[0].output.error,
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
        .await
        .unwrap()
    else {
        panic!("expected a completed response");
    };

    assert!(response.tool_executions[0].output.error.is_none());
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
