mod common;
use common::{ProviderBehavior, ScriptedProvider, tool::EchoTool};
use llm::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct NamedEchoTool {
    name: &'static str,
    execution_count: Arc<AtomicUsize>,
}
impl Tool for NamedEchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.into(),
            description: "named echo".into(),
            input_schema: serde_json::json!({"type":"object","required":["value"]}),
            requires_authorization: false,
        }
    }

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        Ok(ToolPlan {
            call: call.clone(),
            normalized_arguments: call.arguments.clone(),
            required_capabilities: vec![],
            timeout: None,
        })
    }

    fn execute(
        &self,
        plan: ToolPlan,
        _grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(ToolOutput::success(plan.call.id, plan.normalized_arguments)) })
    }
}

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
            .await,
        Err(LlmError::ToolProtocol(_))
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
            .await,
        Err(LlmError::ToolProtocol(_))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
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
        .await
        .unwrap()
    else {
        panic!("expected model-visible failure");
    };
    assert!(matches!(
        response.tool_executions[0].output.error,
        Some(ToolFailure::InvalidArguments(_))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
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
            .await,
        Err(LlmError::LoopLimit(_))
    ));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
}
