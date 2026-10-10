use super::{BasicToolConfiguration, tool_support};
use crate::{
    AuthorizationGrant, BoxFuture, FilesystemAccess, RequiredCapability, Tool, ToolCall,
    ToolDefinition, ToolFailure, ToolOutput, ToolPlan,
};
use os::execute_command_in_directory_with_arguments_and_options;
use serde_json::{Value, json};
use std::{ffi::OsString, sync::Arc};

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
                json!({
                    "program": {"type": "string", "minLength": 1},
                    "arguments": {
                        "type": "array",
                        "items": {"type": "string"},
                        "default": []
                    }
                }),
                &["program"],
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
        let (program, arguments) = parse_command_arguments(&tool_call.arguments)?;
        let normalized_arguments = json!({
            "program": program,
            "arguments": arguments,
        });

        Ok(tool_support::create_tool_plan(
            tool_call,
            normalized_arguments,
            vec![
                tool_support::filesystem_capability(
                    self.configuration.root_directory().to_owned(),
                    FilesystemAccess::Modify,
                ),
                RequiredCapability::Command { program, arguments },
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
            let (program, arguments) = parse_command_arguments(&tool_plan.normalized_arguments)?;

            authorization_grant.claim_capabilities(&[
                RequiredCapability::Filesystem {
                    path: configuration.root_directory().to_owned(),
                    access: FilesystemAccess::Modify,
                },
                RequiredCapability::Command {
                    program: program.clone(),
                    arguments: arguments.clone(),
                },
            ])?;

            let command_entity = execute_command_in_directory_with_arguments_and_options(
                configuration.root_directory(),
                OsString::from(program),
                arguments.into_iter().map(OsString::from),
                configuration.command_options.clone(),
            )
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

fn parse_command_arguments(tool_arguments: &Value) -> Result<(String, Vec<String>), ToolFailure> {
    let program = tool_arguments
        .get("program")
        .and_then(Value::as_str)
        .filter(|program| !program.trim().is_empty())
        .ok_or_else(|| ToolFailure::InvalidArguments("program must be a string".into()))?
        .to_owned();

    let arguments = tool_arguments
        .get("arguments")
        .map_or(Ok(Vec::new()), |arguments| {
            arguments
                .as_array()
                .ok_or_else(|| ToolFailure::InvalidArguments("arguments must be an array".into()))?
                .iter()
                .map(|argument| {
                    argument.as_str().map(str::to_owned).ok_or_else(|| {
                        ToolFailure::InvalidArguments(
                            "every command argument must be a string".into(),
                        )
                    })
                })
                .collect()
        })?;

    Ok((program, arguments))
}
