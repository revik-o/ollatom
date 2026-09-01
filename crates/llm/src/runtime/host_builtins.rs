use super::host::RunHost;
use crate::{LlmError, ProviderRunHost, ToolCall, ToolFailure, ToolOutput};

impl RunHost {
    pub(super) async fn execute_builtin_tool(
        &self,
        tool_call: ToolCall,
    ) -> Result<ToolOutput, LlmError> {
        self.emit_tool_planned(&tool_call).await?;
        self.emit_tool_started(&tool_call).await?;

        let output_content = match tool_call.name.as_str() {
            crate::builtins::ASK_USER_TOOL_NAME => {
                let request =
                    match crate::builtins::parse_ask_user_request(tool_call.arguments.clone()) {
                        Ok(request) => request,
                        Err(error) => {
                            return self
                                .finish_tool_failure(
                                    &tool_call,
                                    ToolFailure::InvalidArguments(error.to_string()),
                                )
                                .await;
                        }
                    };

                serde_json::to_value(self.ask_user(request).await?)
                    .map_err(|error| LlmError::ProviderProtocol(error.to_string()))?
            }
            crate::builtins::INVOKE_SUBAGENT_TOOL_NAME => {
                let request = match crate::builtins::parse_invoke_subagent_request(
                    tool_call.arguments.clone(),
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        return self
                            .finish_tool_failure(
                                &tool_call,
                                ToolFailure::InvalidArguments(error.to_string()),
                            )
                            .await;
                    }
                };

                let outcome = match self.invoke_configured_subagent(request).await {
                    Ok(outcome) => outcome,
                    Err(LlmError::InvalidToolArguments(message)) => {
                        return self
                            .finish_tool_failure(&tool_call, ToolFailure::InvalidArguments(message))
                            .await;
                    }
                    Err(LlmError::Timeout(message)) => {
                        return self
                            .finish_tool_failure(&tool_call, ToolFailure::Timeout(message))
                            .await;
                    }
                    Err(error) => return Err(error),
                };

                serde_json::to_value(outcome)
                    .map_err(|error| LlmError::ProviderProtocol(error.to_string()))?
            }
            _ => {
                return Err(LlmError::ToolProtocol(format!(
                    "unknown built-in tool {}",
                    tool_call.name
                )));
            }
        };

        self.finish_tool_output(
            &tool_call,
            ToolOutput::success(tool_call.id.clone(), output_content),
        )
        .await
    }
}
