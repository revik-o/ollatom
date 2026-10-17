use llm::{
    AuthorizationGrant, BoxFuture, LlmError, Tool, ToolCall, ToolDefinition, ToolFailure, ToolOutput, ToolPlan,
    ToolRegistry,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn malformed_tool_schemas_are_rejected() {
    for input_schema in [
        serde_json::json!({"type":"not-a-json-schema-type"}),
        serde_json::json!({
            "type": "string",
            "minLength": "thousand",
            "maxLength": false,
            "additionalProperties": "no"
        }),
    ] {
        let malformed_definition = ToolDefinition {
            name: "bad".into(),
            description: "bad".into(),
            input_schema,
            authorization_requirement: llm::ToolAuthorizationRequirement::CapabilityPolicy,
        };
        assert!(matches!(
            malformed_definition.validate(),
            Err(LlmError::InvalidToolDefinition(_))
        ));
    }
}

#[test]
fn schema_references_are_rejected_without_retrieval() {
    let definition = ToolDefinition {
        name: "referenced".into(),
        description: "bad".into(),
        input_schema: serde_json::json!({"$ref":"https://example.invalid/schema.json"}),
        authorization_requirement: llm::ToolAuthorizationRequirement::CapabilityPolicy,
    };

    assert!(matches!(
        definition.validate(),
        Err(LlmError::InvalidToolDefinition(message)) if message.contains("references")
    ));
}

struct ChangingDefinitionTool {
    definition_calls: Arc<AtomicUsize>,
}

impl Tool for ChangingDefinitionTool {
    fn definition(&self) -> ToolDefinition {
        let call = self.definition_calls.fetch_add(1, Ordering::SeqCst);
        ToolDefinition {
            name: "changing".into(),
            description: format!("definition {call}"),
            input_schema: serde_json::json!({"type":"object"}),
            authorization_requirement: llm::ToolAuthorizationRequirement::CapabilityPolicy,
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
        _grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        Box::pin(async move { Ok(ToolOutput::success(plan.call.id, serde_json::Value::Null)) })
    }
}

#[test]
fn registry_snapshots_the_definition_at_registration() {
    let definition_calls = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(ChangingDefinitionTool {
            definition_calls: definition_calls.clone(),
        })
        .unwrap();

    assert_eq!(registry.definitions()[0].description, "definition 0");
    assert_eq!(registry.definitions()[0].description, "definition 0");
    assert_eq!(definition_calls.load(Ordering::SeqCst), 1);
}
