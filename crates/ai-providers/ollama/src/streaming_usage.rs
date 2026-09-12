use super::{run::RunProgress, streaming::RoundStreamState};
use llm::UsageSource;
use serde_json::Value;

pub(super) fn record_completed_frame_usage(
    completed_frame: &Value,
    run_progress: &mut RunProgress,
    round_stream_state: &mut RoundStreamState,
) {
    let round_input_tokens = completed_frame
        .get("prompt_eval_count")
        .and_then(Value::as_u64);
    let round_output_tokens = completed_frame.get("eval_count").and_then(Value::as_u64);
    let round_reasoning_tokens = completed_frame
        .get("reasoning_tokens")
        .and_then(Value::as_u64);

    if round_input_tokens.is_some()
        || round_output_tokens.is_some()
        || round_reasoning_tokens.is_some()
    {
        run_progress.usage.source = UsageSource::ApiReported;
        add_usage_value(&mut run_progress.usage.input_tokens, round_input_tokens);
        add_usage_value(&mut run_progress.usage.output_tokens, round_output_tokens);
        add_usage_value(
            &mut run_progress.usage.reasoning_tokens,
            round_reasoning_tokens,
        );
        run_progress.usage.total_tokens = Some(
            run_progress
                .usage
                .input_tokens
                .unwrap_or(0)
                .saturating_add(run_progress.usage.output_tokens.unwrap_or(0)),
        );
    }

    if let Some(round_input_tokens) = round_input_tokens {
        round_stream_state.observed_input_tokens = round_input_tokens;
    }
}

fn add_usage_value(accumulated_value: &mut Option<u64>, additional_value: Option<u64>) {
    if let Some(additional_value) = additional_value {
        *accumulated_value = Some(
            accumulated_value
                .unwrap_or(0)
                .saturating_add(additional_value),
        );
    }
}

pub(super) fn finish_reason_from_native(value: Option<&Value>) -> llm::FinishReason {
    match value.and_then(Value::as_str) {
        Some("length" | "limit") => llm::FinishReason::Length,
        Some("tool_calls" | "tool_call") => llm::FinishReason::ToolCalls,
        Some("content_filter") => llm::FinishReason::ContentFilter,
        Some("error") => llm::FinishReason::Error,
        Some("cancelled") => llm::FinishReason::Cancelled,
        Some("stop") | None => llm::FinishReason::Stop,
        Some(_) => llm::FinishReason::Unknown,
    }
}
