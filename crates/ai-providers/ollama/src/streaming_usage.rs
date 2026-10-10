use super::streaming::{NativeChatFrame, RoundStreamState};
use llm::{AssistantRound, Usage, UsageSource};

pub(super) fn record_completed_frame_usage(
    completed_frame: &NativeChatFrame,
    aggregate_usage: &mut Usage,
    assistant_round: &mut AssistantRound,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), llm::LlmError> {
    let round_input_tokens = completed_frame.prompt_eval_count;
    let round_output_tokens = completed_frame.eval_count;
    let round_reasoning_tokens = completed_frame.reasoning_tokens;

    if round_input_tokens.is_some()
        || round_output_tokens.is_some()
        || round_reasoning_tokens.is_some()
    {
        let accumulated_input_tokens =
            add_usage_value(aggregate_usage.input_tokens, round_input_tokens)?;
        let accumulated_output_tokens =
            add_usage_value(aggregate_usage.output_tokens, round_output_tokens)?;
        let accumulated_reasoning_tokens =
            add_usage_value(aggregate_usage.reasoning_tokens, round_reasoning_tokens)?;

        let accumulated_total_tokens = total_token_count(
            accumulated_input_tokens,
            accumulated_output_tokens,
            accumulated_reasoning_tokens,
        )?;

        let round_total_tokens = total_token_count(
            round_input_tokens,
            round_output_tokens,
            round_reasoning_tokens,
        )?;

        aggregate_usage.source = UsageSource::ApiReported;
        aggregate_usage.input_tokens = accumulated_input_tokens;
        aggregate_usage.output_tokens = accumulated_output_tokens;
        aggregate_usage.reasoning_tokens = accumulated_reasoning_tokens;
        aggregate_usage.total_tokens = Some(accumulated_total_tokens);

        assistant_round.usage.source = UsageSource::ApiReported;
        assistant_round.usage.input_tokens = round_input_tokens;
        assistant_round.usage.output_tokens = round_output_tokens;
        assistant_round.usage.reasoning_tokens = round_reasoning_tokens;
        assistant_round.usage.total_tokens = Some(round_total_tokens);
    }

    if let Some(round_input_tokens) = round_input_tokens {
        round_stream_state.observed_input_tokens = round_input_tokens;
    }

    Ok(())
}

fn add_usage_value(
    accumulated_value: Option<u64>,
    additional_value: Option<u64>,
) -> Result<Option<u64>, llm::LlmError> {
    additional_value.map_or(Ok(accumulated_value), |additional_value| {
        accumulated_value
            .unwrap_or(0)
            .checked_add(additional_value)
            .map(Some)
            .ok_or_else(|| usage_overflow_error("accumulated"))
    })
}

fn total_token_count(
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
) -> Result<u64, llm::LlmError> {
    input_tokens
        .unwrap_or(0)
        .checked_add(output_tokens.unwrap_or(0))
        .and_then(|token_count| token_count.checked_add(reasoning_tokens.unwrap_or(0)))
        .ok_or_else(|| usage_overflow_error("reported"))
}

fn usage_overflow_error(scope: &str) -> llm::LlmError {
    llm::LlmError::ProviderProtocol(format!("{scope} token usage exceeds the supported range"))
}

pub(super) fn finish_reason_from_native(value: Option<&str>) -> llm::FinishReason {
    match value {
        Some("length" | "limit") => llm::FinishReason::Length,
        Some("tool_calls" | "tool_call") => llm::FinishReason::ToolCalls,
        Some("content_filter") => llm::FinishReason::ContentFilter,
        Some("error") => llm::FinishReason::Error,
        Some("cancelled") => llm::FinishReason::Cancelled,
        Some("stop") | None => llm::FinishReason::Stop,
        Some(_) => llm::FinishReason::Unknown,
    }
}
