mod common;

use common::{ProviderBehavior, ScriptedProvider};
use llm::*;
use std::sync::{
    Arc, Mutex,
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
            authorization_requirement: ToolAuthorizationRequirement::CapabilityPolicy,
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
            filesystem_capability: None,
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

struct AuthorizationRequiredTool {
    execution_count: Arc<AtomicUsize>,
}

impl Tool for AuthorizationRequiredTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "unscoped_action".into(),
            description: "an action requiring explicit authorization".into(),
            input_schema: serde_json::json!({"type": "object"}),
            authorization_requirement: ToolAuthorizationRequirement::ExplicitUserApproval,
        }
    }

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        Ok(ToolPlan {
            call: call.clone(),
            normalized_arguments: call.arguments.clone(),
            required_capabilities: vec![],
            filesystem_capability: None,
            timeout: None,
        })
    }

    fn execute(
        &self,
        plan: ToolPlan,
        _authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(ToolOutput::success(plan.call.id, serde_json::json!("done"))) })
    }
}

struct UnapprovedCapabilityClaimTool {
    execution_count: Arc<AtomicUsize>,
}

impl Tool for UnapprovedCapabilityClaimTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "unapproved_claim".into(),
            description: "attempt an unapproved capability claim".into(),
            input_schema: serde_json::json!({"type": "object"}),
            authorization_requirement: ToolAuthorizationRequirement::CapabilityPolicy,
        }
    }

    fn plan(&self, call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        Ok(ToolPlan {
            call: call.clone(),
            normalized_arguments: call.arguments.clone(),
            required_capabilities: Vec::new(),
            filesystem_capability: None,
            timeout: None,
        })
    }

    fn execute(
        &self,
        plan: ToolPlan,
        authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        let execution_count = self.execution_count.clone();

        Box::pin(async move {
            authorization_grant.claim_capabilities(&[RequiredCapability::Network {
                host: "search.example".into(),
            }])?;
            execution_count.fetch_add(1, Ordering::SeqCst);
            Ok(ToolOutput::success(plan.call.id, serde_json::Value::Null))
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

fn runtime_with_unscoped_tool(execution_count: Arc<AtomicUsize>) -> LlmRuntime {
    let provider = ScriptedProvider::new(
        BuiltInProvider::Claude,
        ProviderBehavior::ExecuteToolRound(vec![ToolCall {
            id: "unscoped-1".into(),
            name: "unscoped_action".into(),
            arguments: serde_json::json!({}),
        }]),
    );
    let mut tools = ToolRegistry::new();
    tools
        .register(AuthorizationRequiredTool { execution_count })
        .unwrap();

    LlmRuntime::builder()
        .provider_with_default(provider, "model")
        .tools(tools)
        .build()
        .unwrap()
}

struct ApprovingToolAuthorizer {
    requests: Arc<Mutex<Vec<ApprovalRequest>>>,
}

impl ToolAuthorizer for ApprovingToolAuthorizer {
    fn authorize(
        &self,
        request: ApprovalRequest,
    ) -> BoxFuture<'_, Result<ApprovalDecision, LlmError>> {
        self.requests.lock().unwrap().push(request);
        Box::pin(async { Ok(ApprovalDecision::AllowOnce) })
    }
}

#[path = "authorization_contracts/approval_policy.rs"]
mod approval_policy;
#[path = "authorization_contracts/trusted_paths.rs"]
mod trusted_paths;
