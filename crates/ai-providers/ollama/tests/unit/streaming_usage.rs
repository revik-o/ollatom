use crate::streaming::{NativeChatFrame, RoundStreamState};
use crate::streaming_usage::record_completed_frame_usage;
use llm::{AssistantRound, Usage, UsageSource};

fn completed_frame(frame: &str) -> NativeChatFrame {
    serde_json::from_str(frame).expect("frame should deserialize")
}

#[test]
fn reported_total_includes_reasoning_tokens() {
    let mut aggregate_usage = Usage::default();
    let mut assistant_round = AssistantRound::default();
    let mut round_stream_state = RoundStreamState::default();

    record_completed_frame_usage(
        &completed_frame(r#"{"done":true,"prompt_eval_count":7,"eval_count":2,"reasoning_tokens":3}"#),
        &mut aggregate_usage,
        &mut assistant_round,
        &mut round_stream_state,
    )
    .expect("reported token totals should fit");

    assert_eq!(aggregate_usage.total_tokens, Some(12));
    assert_eq!(assistant_round.usage.total_tokens, Some(12));
    assert_eq!(aggregate_usage.reasoning_tokens, Some(3));
    assert_eq!(assistant_round.usage.source, UsageSource::ApiReported);
}

#[test]
fn reported_total_overflow_is_rejected_without_partial_mutation() {
    let mut aggregate_usage = Usage::default();
    let mut assistant_round = AssistantRound::default();
    let mut round_stream_state = RoundStreamState::default();

    let error = record_completed_frame_usage(
        &completed_frame(r#"{"done":true,"prompt_eval_count":18446744073709551615,"eval_count":1}"#),
        &mut aggregate_usage,
        &mut assistant_round,
        &mut round_stream_state,
    )
    .expect_err("overflowing token totals should fail");

    assert!(matches!(error, llm::LlmError::ProviderProtocol(_)));
    assert_eq!(aggregate_usage, Usage::default());
    assert_eq!(assistant_round.usage, Usage::default());
}

#[test]
fn accumulated_reported_field_overflow_is_rejected() {
    let mut aggregate_usage = Usage::default();
    let mut assistant_round = AssistantRound::default();
    let mut round_stream_state = RoundStreamState::default();

    record_completed_frame_usage(
        &completed_frame(r#"{"done":true,"prompt_eval_count":18446744073709551615}"#),
        &mut aggregate_usage,
        &mut assistant_round,
        &mut round_stream_state,
    )
    .expect("first round token count should fit");

    let error = record_completed_frame_usage(
        &completed_frame(r#"{"done":true,"prompt_eval_count":1}"#),
        &mut aggregate_usage,
        &mut assistant_round,
        &mut round_stream_state,
    )
    .expect_err("accumulated token counts should fail before wrapping");

    assert!(matches!(error, llm::LlmError::ProviderProtocol(_)));
    assert_eq!(aggregate_usage.input_tokens, Some(u64::MAX));
    assert_eq!(aggregate_usage.total_tokens, Some(u64::MAX));
}
