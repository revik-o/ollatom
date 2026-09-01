use llm::{LlmError, ToolDefinition};

#[test]
fn malformed_tool_schemas_are_rejected() {
    let malformed_definition = ToolDefinition {
        name: "bad".into(),
        description: "bad".into(),
        input_schema: serde_json::json!({"type":"not-a-json-schema-type"}),
        requires_authorization: false,
    };
    assert!(matches!(
        malformed_definition.validate(),
        Err(LlmError::InvalidToolDefinition(_))
    ));
}
