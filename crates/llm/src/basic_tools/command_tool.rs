use super::{BasicToolConfiguration, tool_support};
use crate::{
    AuthorizationGrant, BoxFuture, FilesystemAccess, RequiredCapability, Tool, ToolCall,
    ToolDefinition, ToolFailure, ToolOutput, ToolPlan,
};
use os::{execute_command_in_directory, parse_command};
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct CommandExecutionTool {
    configuration: Arc<BasicToolConfiguration>,
    definition: ToolDefinition,
}

impl CommandExecutionTool {
    pub(super) fn new(configuration: Arc<BasicToolConfiguration>) -> Self {
        Self {
            configuration,
            definition: tool_support::create_tool_definition(
                "execute_command",
                "Run a non-interactive command in the configured working directory.",
                json!({"command": {"type": "string", "minLength": 1}}),
                &["command"],
            ),
        }
    }
}

impl Tool for CommandExecutionTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    fn plan(&self, tool_call: &ToolCall) -> Result<ToolPlan, ToolFailure> {
        tool_support::validate_tool_call(&self.definition, tool_call)?;

        let command_text = parse_command_text(&tool_call.arguments)?;
        let command_invocation = parse_command(command_text)
            .map_err(|error| ToolFailure::InvalidArguments(error.to_string()))?;

        Ok(tool_support::create_tool_plan(
            tool_call,
            tool_call.arguments.clone(),
            vec![
                tool_support::filesystem_capability(
                    self.configuration.root_directory().to_owned(),
                    FilesystemAccess::Modify,
                ),
                RequiredCapability::Command {
                    program: command_invocation.program,
                    arguments: command_invocation.arguments,
                },
            ],
            Some(self.configuration.command_timeout),
        ))
    }

    fn execute(
        &self,
        tool_plan: ToolPlan,
        authorization_grant: AuthorizationGrant,
    ) -> BoxFuture<'static, Result<ToolOutput, ToolFailure>> {
        let configuration = self.configuration.clone();

        Box::pin(async move {
            let command_text = parse_command_text(&tool_plan.normalized_arguments)?;
            let command_entity =
                execute_command_in_directory(configuration.root_directory(), command_text)
                    .map_err(|error| ToolFailure::Execution(error.to_string()))?;
            let mut command_entity = Box::pin(command_entity);

            tokio::select! {
                command_result = &mut command_entity => {
                    let command_outcome = command_result.map_err(|error| ToolFailure::Execution(error.to_string()))?;
                    Ok(ToolOutput::success(tool_plan.call.id, json!({"tool": "execute_command", "result": command_outcome})))
                }
                () = authorization_grant.stop_token().cancelled() => {
                    command_entity.as_ref().get_ref().terminate().map_err(|error| ToolFailure::Execution(error.to_string()))?;
                    Err(ToolFailure::Timeout("command cancelled".into()))
                }
            }
        })
    }
}

fn parse_command_text(tool_arguments: &Value) -> Result<&str, ToolFailure> {
    tool_arguments
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolFailure::InvalidArguments("command must be a string".into()))
}
