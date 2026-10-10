use super::{
    content::NativeToolCall,
    streaming_tools::merge_native_tool_calls,
    streaming_usage::{finish_reason_from_native, record_completed_frame_usage},
    transport::network_error,
};
use futures_util::StreamExt;
use llm::{
    AssistantRound, ControlledFutureOutcome, FinishReason, LlmError, ProviderRunHost, RunEvent,
    StopToken, Usage,
};
use reqwest::Response;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc, time::Duration};

const MAXIMUM_ERROR_MESSAGE_CHARACTERS: usize = 4096;
const MAXIMUM_FRAME_BYTE_COUNT: usize = 1024 * 1024;

mod round;
pub(super) use round::stream_round;

#[derive(Debug, Default, Deserialize)]
pub(super) struct NativeChatFrame {
    #[serde(default, deserialize_with = "deserialize_present_value")]
    pub(super) error: Option<NativeChatError>,
    #[serde(default)]
    pub(super) message: NativeChatMessageDelta,
    #[serde(default)]
    pub(super) done: bool,
    #[serde(default)]
    pub(super) done_reason: Option<String>,
    #[serde(default)]
    pub(super) prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub(super) eval_count: Option<u64>,
    #[serde(default)]
    pub(super) reasoning_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct NativeChatMessageDelta {
    #[serde(default)]
    pub(super) content: String,
    #[serde(default)]
    pub(super) thinking: String,
    #[serde(default)]
    pub(super) tool_calls: Vec<NativeToolCall>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum NativeChatError {
    Message(String),
    Details(NativeChatErrorDetails),
}

#[derive(Debug, Deserialize)]
pub(super) struct NativeChatErrorDetails {
    #[serde(default)]
    message: Option<String>,
}

impl NativeChatError {
    fn message(&self) -> Option<&str> {
        match self {
            Self::Message(message) => Some(message),
            Self::Details(details) => details.message.as_deref(),
        }
    }
}

fn deserialize_present_value<'de, Deserializer, ValueType>(
    deserializer: Deserializer,
) -> Result<Option<ValueType>, Deserializer::Error>
where
    Deserializer: serde::Deserializer<'de>,
    ValueType: Deserialize<'de>,
{
    ValueType::deserialize(deserializer).map(Some)
}

pub(super) struct RoundResult {
    pub(super) tool_calls: Vec<NativeToolCall>,
    pub(super) finish_reason: FinishReason,
    pub(super) observed_input_tokens: u64,
}

pub(super) struct RoundStreamState {
    pub(super) tool_calls: BTreeMap<usize, NativeToolCall>,
    pub(super) is_complete: bool,
    pub(super) finish_reason: FinishReason,
    pub(super) observed_input_tokens: u64,
}

impl Default for RoundStreamState {
    fn default() -> Self {
        Self {
            tool_calls: BTreeMap::new(),
            is_complete: false,
            finish_reason: FinishReason::Stop,
            observed_input_tokens: 0,
        }
    }
}

async fn process_frame(
    frame: &NativeChatFrame,
    host: &Arc<dyn ProviderRunHost>,
    assistant_round: &mut AssistantRound,
    aggregate_usage: &mut Usage,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    if let Some(error) = &frame.error {
        return Err(native_error_frame(error));
    }

    append_response_delta(&frame.message.content, host, assistant_round).await?;
    append_reasoning_delta(&frame.message.thinking, host, assistant_round).await?;
    merge_native_tool_calls(
        &frame.message.tool_calls,
        &mut round_stream_state.tool_calls,
    )?;

    if frame.done {
        round_stream_state.is_complete = true;
        round_stream_state.finish_reason = finish_reason_from_native(frame.done_reason.as_deref());
        record_completed_frame_usage(frame, aggregate_usage, assistant_round, round_stream_state)?;
    }

    Ok(())
}

fn native_error_frame(error: &NativeChatError) -> LlmError {
    let error_message = error
        .message()
        .map(ToString::to_string)
        .unwrap_or_else(|| "Ollama returned an error frame".into());

    LlmError::Provider(
        error_message
            .chars()
            .take(MAXIMUM_ERROR_MESSAGE_CHARACTERS)
            .collect(),
    )
}

async fn append_response_delta(
    response_delta: &str,
    host: &Arc<dyn ProviderRunHost>,
    assistant_round: &mut AssistantRound,
) -> Result<(), LlmError> {
    if response_delta.is_empty() {
        return Ok(());
    }

    assistant_round.text.push_str(response_delta);
    host.emit(RunEvent::ResponseDelta(response_delta.into()))
        .await
}

async fn append_reasoning_delta(
    reasoning_delta: &str,
    host: &Arc<dyn ProviderRunHost>,
    assistant_round: &mut AssistantRound,
) -> Result<(), LlmError> {
    if reasoning_delta.is_empty() {
        return Ok(());
    }

    assistant_round
        .visible_reasoning
        .get_or_insert_with(String::new)
        .push_str(reasoning_delta);

    host.emit(RunEvent::ReasoningSummaryDelta(reasoning_delta.into()))
        .await
}
