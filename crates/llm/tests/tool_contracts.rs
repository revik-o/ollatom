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
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"value": {"type": "string"}},
                "required": ["value"]
            }),
            authorization_requirement: ToolAuthorizationRequirement::CapabilityPolicy,
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
        _grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(ToolOutput::success(plan.call.id, plan.normalized_arguments)) })
    }
}

#[path = "tool_contracts/call_validation.rs"]
mod call_validation;
#[path = "tool_contracts/tool_results.rs"]
mod tool_results;
