use super::host::RunHost;
use crate::{LlmError, RunEvent, ToolCall, ToolEvent, ToolFailure, ToolOutput};
impl RunHost {
    pub(super) async fn validate_tool_round(
        &self,
        round: u16,
        tool_calls: &[ToolCall],
    ) -> Result<(), LlmError> {
        if self.stop.is_stopped() {
            return Err(LlmError::Cancelled);
        }

        if tool_calls
            .iter()
            .any(|tool_call| tool_call.id.trim().is_empty())
        {
            return Err(LlmError::ToolProtocol(
                "tool call ID must be non-empty".into(),
            ));
        }

        let mut host_state = self.state.lock().await;

        if round != host_state.round {
            return Err(LlmError::ProviderProtocol(format!(
                "tool calls for unopened round {round}"
            )));
        }

        let tool_call_count = u16::try_from(tool_calls.len())
            .map_err(|_| LlmError::LoopLimit("tool calls per round".into()))?;

        if tool_call_count > self.limits.tool_calls_per_round {
            return Err(LlmError::LoopLimit("tool calls per round".into()));
        }

        let projected_total_calls = host_state
            .total_calls
            .checked_add(tool_call_count)
            .ok_or_else(|| LlmError::LoopLimit("total tool calls".into()))?;

        if projected_total_calls > self.limits.total_tool_calls {
            return Err(LlmError::LoopLimit("total tool calls".into()));
        }

        host_state.total_calls = projected_total_calls;

        Ok(())
    }

    pub(super) async fn preflight_tool_round(
        &self,
        tool_calls: &[ToolCall],
    ) -> Result<(), LlmError> {
        for tool_call in tool_calls {
            self.ensure_active()?;

            if self.cached_tool_output(tool_call).await?.is_some() {
                continue;
            }

            if crate::builtins::built_in_tool_definition(&tool_call.name).is_some() {
                if !self.is_tool_selected(&tool_call.name) {
                    return Err(LlmError::ToolProtocol(format!(
                        "unadvertised tool {}",
                        tool_call.name
                    )));
                }
                continue;
            }

            if !self.is_tool_selected(&tool_call.name) {
                return Err(LlmError::ToolProtocol(format!(
                    "unadvertised tool {}",
                    tool_call.name
                )));
            }

            let Some(tool) = self.tools.get(&tool_call.name) else {
                return Err(LlmError::ToolProtocol(format!(
                    "unknown tool {}",
                    tool_call.name
                )));
            };

            let _ = tool;
        }

        Ok(())
    }

    pub(super) async fn execute_tool_call(
        &self,
        tool_call: ToolCall,
    ) -> Result<ToolOutput, LlmError> {
        self.ensure_active()?;

        if let Some(cached_output) = self.cached_tool_output(&tool_call).await? {
            self.emit_undelivered_finished_event(&tool_call).await?;
            return Ok(cached_output);
        }

        if !self.is_tool_selected(&tool_call.name) {
            return Err(LlmError::ToolProtocol(format!(
                "unadvertised tool {}",
                tool_call.name
            )));
        }

        if crate::builtins::built_in_tool_definition(&tool_call.name).is_some() {
            return self.execute_builtin_tool(tool_call).await;
        }

        let Some(tool) = self.tools.get(&tool_call.name) else {
            return Err(LlmError::ToolProtocol(format!(
                "unknown tool {}",
                tool_call.name
            )));
        };
        let argument_validation = self
            .tools
            .validate_arguments(&tool_call.name, &tool_call.arguments);

        if let Err(failure) = argument_validation {
            return self.finish_tool_failure(&tool_call, failure).await;
        }

        let tool_plan = match tool.plan(&tool_call) {
            Ok(tool_plan) => tool_plan,
            Err(failure) => return self.finish_tool_failure(&tool_call, failure).await,
        };

        if tool_plan.call != tool_call {
            return Err(LlmError::ToolProtocol(format!(
                "tool {} changed the call while planning",
                tool_call.name
            )));
        }

        self.emit_tool_planned(&tool_call).await?;

        let (tool_token, tool_stop) = crate::cancellation::stop_pair();
        let tool_stop = crate::cancellation::StopOnDrop::new(tool_stop);
        let authorization_grant = match self.authorize(&tool_plan, tool_token).await? {
            Some(authorization_grant) => authorization_grant,
            None => {
                return self
                    .finish_tool_failure(&tool_call, ToolFailure::Denied("not authorized".into()))
                    .await;
            }
        };

        self.ensure_active()?;
        self.emit_tool_started(&tool_call).await?;

        let requested_timeout = tool_plan
            .timeout
            .unwrap_or_else(|| std::time::Duration::from_millis(self.limits.tool_timeout_ms));

        let timeout = self
            .overall_deadline
            .and_then(|deadline| deadline.checked_duration_since(std::time::Instant::now()))
            .map_or(requested_timeout, |remaining| {
                requested_timeout.min(remaining)
            });

        let tool_output = tokio::select! {
            _ = self.stop.cancelled() => {
                tool_stop.stop();

                let _ = self.events.emit(RunEvent::Tool(ToolEvent::Cancelled {
                    call_id: tool_call.id.clone(),
                    reason: "run cancelled".into(),
                })).await;

                return Err(LlmError::Cancelled);
            },
            _ = tokio::time::sleep(timeout) => {
                tool_stop.stop();

                ToolOutput::failure(
                    tool_call.id.clone(),
                    ToolFailure::Timeout(format!("tool exceeded {timeout:?}")),
                )
            },
            execution_result = tool.execute(tool_plan, authorization_grant) => {
                match execution_result {
                    Ok(tool_output) => tool_output,
                    Err(failure) => ToolOutput::failure(tool_call.id.clone(), failure),
                }
            },
        };

        self.finish_tool_output(&tool_call, tool_output).await
    }
}
