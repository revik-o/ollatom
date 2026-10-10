use super::BasicToolConfiguration;
use crate::{ToolDefinition, ToolFailure, ToolPlan};
use serde_json::{Value, json};
use std::path::PathBuf;

pub(super) fn create_tool_definition(
    name: &str,
    description: &str,
    properties: Value,
    required_property_names: &[&str],
) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: description.into(),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required_property_names,
            "additionalProperties": false
        }),
        authorization_requirement: crate::ToolAuthorizationRequirement::CapabilityPolicy,
    }
}

pub(super) fn validate_tool_call(
    tool_definition: &ToolDefinition,
    tool_call: &crate::ToolCall,
) -> Result<(), ToolFailure> {
    crate::tools::validate_arguments(&tool_definition.input_schema, &tool_call.arguments)
}

pub(super) fn resolve_workspace_file_argument(
    configuration: &BasicToolConfiguration,
    tool_arguments: &Value,
    argument_name: &str,
) -> Result<filesystem::WorkspaceFile, ToolFailure> {
    let relative_path = tool_arguments
        .get(argument_name)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolFailure::InvalidArguments(format!("{argument_name} must be a string"))
        })?;

    configuration
        .workspace_directory()
        .file(relative_path)
        .map_err(|error| ToolFailure::InvalidArguments(error.to_string()))
}

pub(super) fn filesystem_capability(
    filesystem_path: PathBuf,
    filesystem_access: crate::FilesystemAccess,
) -> crate::RequiredCapability {
    crate::RequiredCapability::Filesystem {
        path: filesystem_path,
        access: filesystem_access,
    }
}

pub(super) fn create_tool_plan(
    tool_call: &crate::ToolCall,
    normalized_arguments: Value,
    required_capabilities: Vec<crate::RequiredCapability>,
    timeout: Option<std::time::Duration>,
) -> ToolPlan {
    create_tool_plan_with_filesystem_capability(
        tool_call,
        normalized_arguments,
        required_capabilities,
        timeout,
        None,
    )
}

pub(super) fn create_tool_plan_with_filesystem_capability(
    tool_call: &crate::ToolCall,
    normalized_arguments: Value,
    required_capabilities: Vec<crate::RequiredCapability>,
    timeout: Option<std::time::Duration>,
    filesystem_capability: Option<filesystem::WorkspaceCapability>,
) -> ToolPlan {
    ToolPlan {
        call: tool_call.clone(),
        normalized_arguments,
        required_capabilities,
        timeout,
        filesystem_capability,
    }
}
