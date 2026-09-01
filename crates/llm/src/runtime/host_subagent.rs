use super::host::RunHost;
use crate::{InvokeSubagentRequest, LlmError, SubagentOutcome};

impl RunHost {
    pub(super) async fn invoke_configured_subagent(
        &self,
        subagent_request: InvokeSubagentRequest,
    ) -> Result<SubagentOutcome, LlmError> {
        if self.stop.is_stopped() {
            return Err(LlmError::Cancelled);
        }

        let subagent_profile = self
            .profiles
            .get(&subagent_request.profile)
            .cloned()
            .ok_or_else(|| {
                LlmError::InvalidToolArguments(format!(
                    "unknown subagent profile {}",
                    subagent_request.profile.as_str()
                ))
            })?;
        {
            let mut host_state = self.state.lock().await;
            let reached_depth_limit = self.limits.subagent_depth == 0;
            let reached_child_limit = host_state.children >= self.limits.child_subagents;
            if reached_depth_limit || reached_child_limit {
                return Err(LlmError::LoopLimit("child subagent limit".into()));
            }
            host_state.children += 1;
        }
        let subagent_runner = self
            .subagent_runner
            .as_ref()
            .ok_or_else(|| LlmError::InvalidRequest("subagent runner is not configured".into()))?;
        let (child_stop_token, child_stop_handle) = crate::cancellation::stop_pair();
        let child_stop_guard = crate::cancellation::StopOnDrop::new(child_stop_handle);
        let inherited_context = if subagent_profile.allow_context {
            self.parent_context.clone()
        } else {
            Vec::new()
        };
        let remaining_depth = self
            .limits
            .subagent_depth
            .min(subagent_profile.max_depth)
            .saturating_sub(1);
        let subagent_future = subagent_runner.invoke(
            subagent_request,
            subagent_profile.clone(),
            inherited_context,
            child_stop_token,
            remaining_depth,
        );
        let subagent_outcome = tokio::select! {
            _ = self.stop.cancelled() => {
                child_stop_guard.stop();
                Err(LlmError::Cancelled)
            },
            _ = wait_for_timeout(subagent_profile.timeout) => {
                child_stop_guard.stop();
                Err(LlmError::Timeout("subagent timed out".into()))
            },
            subagent_result = subagent_future => subagent_result,
        }?;

        self.state
            .lock()
            .await
            .child_usage
            .push(subagent_outcome.usage.clone());

        Ok(subagent_outcome)
    }
}

async fn wait_for_timeout(timeout: Option<std::time::Duration>) {
    match timeout {
        Some(timeout) => tokio::time::sleep(timeout).await,
        None => std::future::pending::<()>().await,
    }
}
