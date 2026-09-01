use crate::{AskUserRequest, InvokeSubagentRequest, LlmError, ToolDefinition};

pub const ASK_USER_TOOL_NAME: &str = "ask_user";
pub const INVOKE_SUBAGENT_TOOL_NAME: &str = "invoke_subagent";

pub fn built_in_tool_definition(name: &str) -> Option<ToolDefinition> {
    match name {
        ASK_USER_TOOL_NAME => Some(ToolDefinition {
            name: ASK_USER_TOOL_NAME.into(),
            description: "Ask the user one or more typed questions and wait for their answers."
                .into(),
            input_schema: serde_json::json!({"type":"object","required":["questions"],"properties":{"questions":{"type":"array"}}}),
            requires_authorization: false,
        }),
        INVOKE_SUBAGENT_TOOL_NAME => Some(ToolDefinition {
            name: INVOKE_SUBAGENT_TOOL_NAME.into(),
            description: "Invoke a developer-configured subagent profile for a bounded task."
                .into(),
            input_schema: serde_json::json!({"type":"object","required":["task","profile"],"properties":{"task":{"type":"string"},"profile":{"type":"string"}}}),
            requires_authorization: false,
        }),
        _ => None,
    }
}

pub fn parse_ask_user_request(arguments: serde_json::Value) -> Result<AskUserRequest, LlmError> {
    parse_and_validate_tool_request(arguments)
}
pub fn parse_invoke_subagent_request(
    arguments: serde_json::Value,
) -> Result<InvokeSubagentRequest, LlmError> {
    parse_and_validate_tool_request(arguments)
}

trait ValidatedToolRequest: serde::de::DeserializeOwned {
    fn validate_request(&self) -> Result<(), LlmError>;
}
impl ValidatedToolRequest for AskUserRequest {
    fn validate_request(&self) -> Result<(), LlmError> {
        self.validate()
    }
}
impl ValidatedToolRequest for InvokeSubagentRequest {
    fn validate_request(&self) -> Result<(), LlmError> {
        self.validate()
    }
}

fn parse_and_validate_tool_request<Request>(
    arguments: serde_json::Value,
) -> Result<Request, LlmError>
where
    Request: ValidatedToolRequest,
{
    let request: Request = serde_json::from_value(arguments)
        .map_err(|error| LlmError::InvalidToolArguments(error.to_string()))?;

    request.validate_request()?;

    Ok(request)
}
