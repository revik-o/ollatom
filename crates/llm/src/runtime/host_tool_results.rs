use super::host::{CompletedToolCall, RunHost};
use crate::{LlmError, RunEvent, ToolCall, ToolEvent, ToolFailure, ToolOutput};

impl RunHost {
    pub(super) async fn flush_pending_finished_events(&self) -> Result<(), LlmError> {
        let pending_calls = {
            let host_state = self.state.lock().await;

            host_state
                .completed_calls
                .iter()
                .filter(|(_, completed_call)| !completed_call.finished_event_was_delivered)
                .map(|(call_id, completed_call)| ToolCall {
                    id: call_id.clone(),
                    name: completed_call.tool_name.clone(),
                    arguments: completed_call.arguments.clone(),
                })
                .collect::<Vec<_>>()
        };

        for tool_call in pending_calls {
            self.emit_undelivered_finished_event(&tool_call).await?;
        }

        Ok(())
    }

    pub(super) async fn cached_tool_output(&self, tool_call: &ToolCall) -> Result<Option<ToolOutput>, LlmError> {
        let host_state = self.state.lock().await;

        match host_state.completed_calls.get(&tool_call.id) {
            Some(completed_call)
                if cached_call_matches(tool_call, &completed_call.tool_name, &completed_call.arguments) =>
            {
                Ok(Some(completed_call.output.clone()))
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
        let (durable_acknowledged, delivery_result) = self
            .events
            .emit_with_durable_ack(RunEvent::Tool(ToolEvent::Started {
                call: tool_call.clone(),
            }))
            .await;

        durable_acknowledged.then_some(Ok(())).unwrap_or(delivery_result)
    }

    pub(super) async fn finish_tool_output(
        &self,
        tool_call: &ToolCall,
        tool_output: ToolOutput,
    ) -> Result<ToolOutput, LlmError> {
        if tool_output.call_id() != tool_call.id {
            return Err(LlmError::ToolProtocol(format!(
                "tool {} returned output for call {}",
                tool_call.name,
                tool_output.call_id()
            )));
        }

        self.record_tool_output(tool_call, &tool_output).await;
        self.emit_undelivered_finished_event(tool_call).await?;

        Ok(tool_output)
    }

    pub(super) async fn finish_tool_failure(
        &self,
        tool_call: &ToolCall,
        failure: ToolFailure,
    ) -> Result<ToolOutput, LlmError> {
        self.finish_tool_output(tool_call, ToolOutput::failure(tool_call.id.clone(), failure))
            .await
    }

    async fn record_tool_output(&self, tool_call: &ToolCall, tool_output: &ToolOutput) {
        let mut host_state = self.state.lock().await;

        let is_first_output = host_state
            .completed_calls
            .insert(
                tool_call.id.clone(),
                CompletedToolCall {
                    tool_name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                    output: tool_output.clone(),
                    finished_event_was_delivered: false,
                },
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

    pub(super) async fn emit_undelivered_finished_event(&self, tool_call: &ToolCall) -> Result<(), LlmError> {
        let output = {
            let host_state = self.state.lock().await;
            let Some(completed_call) = host_state.completed_calls.get(&tool_call.id) else {
                return Err(LlmError::ToolProtocol(format!(
                    "tool call {} has no completed output",
                    tool_call.id
                )));
            };

            if !cached_call_matches(tool_call, &completed_call.tool_name, &completed_call.arguments) {
                return Err(LlmError::ToolProtocol(format!(
                    "duplicate call id {} changed tool name or arguments",
                    tool_call.id
                )));
            }

            if completed_call.finished_event_was_delivered {
                return Ok(());
            }

            completed_call.output.clone()
        };

        let (durable_acknowledged, delivery_result) = self
            .events
            .emit_with_durable_ack(RunEvent::Tool(ToolEvent::Finished { output }))
            .await;

        if durable_acknowledged {
            let mut host_state = self.state.lock().await;
            if let Some(completed_call) = host_state.completed_calls.get_mut(&tool_call.id) {
                completed_call.finished_event_was_delivered = true;
            }
        }

        delivery_result?;

        Ok(())
    }

    pub(super) async fn take_child_usage(&self) -> Vec<crate::Usage> {
        std::mem::take(&mut self.state.lock().await.child_usage)
    }

    pub(super) async fn take_tool_records(&self) -> Vec<crate::ToolExecutionRecord> {
        std::mem::take(&mut self.state.lock().await.tool_records)
    }
}

fn cached_call_matches(tool_call: &ToolCall, cached_tool_name: &str, cached_arguments: &serde_json::Value) -> bool {
    cached_tool_name == tool_call.name && cached_arguments == &tool_call.arguments
}
