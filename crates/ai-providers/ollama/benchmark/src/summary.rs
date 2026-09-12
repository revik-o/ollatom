use llm::{FinishReason, LlmError, LlmRunOutcome};
use serde_json::json;

pub(crate) fn run_outcome_summary(
    run_outcome: &Result<LlmRunOutcome, LlmError>,
) -> serde_json::Value {
    match run_outcome {
        Ok(LlmRunOutcome::Completed(response)) => {
            json!({"status": "completed", "finish_reason": finish_reason_name(response.finish_reason), "text": response.text, "visible_reasoning": response.visible_reasoning, "usage": response.usage})
        }
        Ok(LlmRunOutcome::Cancelled(response)) => {
            json!({"status": "cancelled", "text": response.text, "visible_reasoning": response.visible_reasoning, "usage": response.usage})
        }
        Err(error) => json!({"status": "failed", "error": error.to_string()}),
    }
}

fn finish_reason_name(finish_reason: FinishReason) -> &'static str {
    match finish_reason {
        FinishReason::Stop => "stop",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::Length => "length",
        FinishReason::ContentFilter => "content_filter",
        FinishReason::Error => "error",
        FinishReason::Cancelled => "cancelled",
        FinishReason::Unknown => "unknown",
    }
}
