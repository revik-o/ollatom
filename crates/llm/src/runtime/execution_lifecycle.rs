use super::LlmRuntime;
use crate::{LlmError, LlmRunOutcome, RunEvent};

pub(crate) fn terminal_event_for(result: &Result<LlmRunOutcome, LlmError>) -> RunEvent {
    match result {
        Ok(LlmRunOutcome::Completed(_)) => RunEvent::Completed,
        Ok(LlmRunOutcome::Cancelled(_)) | Err(LlmError::Cancelled) => RunEvent::Cancelled,
        Err(error) => RunEvent::Failed(error.to_string()),
    }
}

pub(crate) struct ActiveRunGuard {
    runtime: Option<LlmRuntime>,
    run_id: crate::RunId,
}

impl ActiveRunGuard {
    pub(crate) fn new(runtime: Option<LlmRuntime>, run_id: crate::RunId) -> Self {
        Self { runtime, run_id }
    }
}

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        if let Some(runtime) = &self.runtime {
            runtime.unregister_run(self.run_id);
        }
    }
}
