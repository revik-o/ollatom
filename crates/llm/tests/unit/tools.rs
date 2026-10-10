use crate::BoxFuture;
use crate::tools::{
    AuthorizationGrant, Tool, ToolAuthorizationRequirement, ToolCall, ToolDefinition, ToolFailure,
    ToolOutput, ToolPlan, ToolRegistry,
};

struct SchemaValidationTool {
    definition: ToolDefinition,
}

impl Tool for SchemaValidationTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn plan(&self, _call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        Err(ToolFailure::Execution("planning is not used".into()))
    }

    fn execute(
        &self,
        _plan: ToolPlan,
        _grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        Box::pin(async { Err(ToolFailure::Execution("execution is not used".into())) })
    }
}

#[test]
fn compiled_schema_validation_preserves_large_integer_precision() {
    let mut tool_registry = ToolRegistry::new();
    tool_registry
        .register(SchemaValidationTool {
            definition: ToolDefinition {
                name: "integer_precision".into(),
                description: "validate an exact integer boundary".into(),
                input_schema: serde_json::json!({
                    "type": "integer",
                    "maximum": 9_007_199_254_740_992_u64
                }),
                authorization_requirement: ToolAuthorizationRequirement::CapabilityPolicy,
            },
        })
        .unwrap();

    assert!(
        tool_registry
            .validate_arguments(
                "integer_precision",
                &serde_json::json!(9_007_199_254_740_992_u64)
            )
            .is_ok()
    );
    assert!(
        tool_registry
            .validate_arguments(
                "integer_precision",
                &serde_json::json!(9_007_199_254_740_993_u64)
            )
            .is_err()
    );
}
