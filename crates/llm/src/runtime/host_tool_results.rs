use super::host::RunHost;
use crate::{LlmError, RunEvent, ToolCall, ToolEvent, ToolFailure, ToolOutput};

impl RunHost {
    pub(super) async fn cached_tool_output(
        &self,
        tool_call: &ToolCall,
    ) -> Result<Option<ToolOutput>, LlmError> {
        let host_state = self.state.lock().await;

        match host_state.completed_calls.get(&tool_call.id) {
            Some((tool_name, arguments, output))
                if cached_call_matches(tool_call, tool_name, arguments) =>
            {
                Ok(Some(output.clone()))
            }
            Some(_) => Err(LlmError::ToolProtocol(format!(
                "duplicate call id {} changed tool name or arguments",
                tool_call.id
            ))),
            None => Ok(None),
        }
    }

    pub(super) async fn emit_tool_planned(&self, tool_call: &ToolCall) -> Result<(), LlmError> {
        self.events
            .emit(RunEvent::Tool(ToolEvent::Planned {
                call: tool_call.clone(),
            }))
            .await
    }

    pub(super) async fn emit_tool_started(&self, tool_call: &ToolCall) -> Result<(), LlmError> {
        self.events
            .emit(RunEvent::Tool(ToolEvent::Started {
                call: tool_call.clone(),
            }))
            .await
    }

    pub(super) async fn finish_tool_output(
        &self,
        tool_call: &ToolCall,
        tool_output: ToolOutput,
    ) -> Result<ToolOutput, LlmError> {
        if tool_output.call_id != tool_call.id {
            return Err(LlmError::ToolProtocol(format!(
                "tool {} returned output for call {}",
                tool_call.name, tool_output.call_id
            )));
        }
        self.record_tool_output(tool_call, &tool_output).await;
        self.events
            .emit(RunEvent::Tool(ToolEvent::Finished {
                output: tool_output.clone(),
            }))
            .await?;

        Ok(tool_output)
    }

    pub(super) async fn finish_tool_failure(
        &self,
        tool_call: &ToolCall,
        failure: ToolFailure,
    ) -> Result<ToolOutput, LlmError> {
        self.finish_tool_output(
            tool_call,
            ToolOutput::failure(tool_call.id.clone(), failure),
        )
        .await
    }

    async fn record_tool_output(&self, tool_call: &ToolCall, tool_output: &ToolOutput) {
        let mut host_state = self.state.lock().await;

        let is_first_output = host_state
            .completed_calls
            .insert(
                tool_call.id.clone(),
                (
                    tool_call.name.clone(),
                    tool_call.arguments.clone(),
                    tool_output.clone(),
                ),
            )
            .is_none();
        if is_first_output {
            host_state.tool_records.push(crate::ToolExecutionRecord {
                call_id: tool_call.id.clone(),
                name: tool_call.name.clone(),
                output: tool_output.clone(),
            });
        }
    }

    pub(super) async fn take_child_usage(&self) -> Vec<crate::Usage> {
        std::mem::take(&mut self.state.lock().await.child_usage)
    }

    pub(super) async fn take_tool_records(&self) -> Vec<crate::ToolExecutionRecord> {
        std::mem::take(&mut self.state.lock().await.tool_records)
    }
}

fn cached_call_matches(
    tool_call: &ToolCall,
    cached_tool_name: &str,
    cached_arguments: &serde_json::Value,
) -> bool {
    cached_tool_name == tool_call.name && cached_arguments == &tool_call.arguments
}
