use llm::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub struct EchoTool {
    pub execution_count: Arc<AtomicUsize>,
}

impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "echo".into(),
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
        _authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        self.execution_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { Ok(ToolOutput::success(plan.call.id, plan.normalized_arguments)) })
    }
}
