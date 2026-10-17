use llm::{LlmError, LlmRunOutcome};
use serde_json::json;

pub(crate) fn run_outcome_summary(run_outcome: &Result<LlmRunOutcome, LlmError>) -> serde_json::Value {
    match run_outcome {
        Ok(LlmRunOutcome::Completed(response)) => {
            json!({"status": "completed", "assistant_rounds": response.assistant_rounds, "tool_executions": response.tool_executions, "usage": response.usage})
        }
        Ok(LlmRunOutcome::Cancelled(response)) => {
            json!({"status": "cancelled", "assistant_rounds": response.assistant_rounds, "tool_executions": response.tool_executions, "usage": response.usage})
        }
        Ok(LlmRunOutcome::Failed(failed_run)) => {
            json!({"status": "failed", "error": failed_run.error.to_string(), "assistant_rounds": failed_run.partial_response.assistant_rounds, "usage": failed_run.partial_response.usage, "tool_executions": failed_run.partial_response.tool_executions, "terminal_event_delivered": failed_run.terminal_event_delivered})
        }
        Err(error) => json!({"status": "failed", "error": error.to_string()}),
    }
}
