use super::{
    content::NativeToolCall,
    run::RunProgress,
    streaming_tools::merge_native_tool_calls,
    streaming_usage::{finish_reason_from_native, record_completed_frame_usage},
    transport::network_error,
};
use futures_util::StreamExt;
use llm::{FinishReason, LlmError, ProviderRunHost, RunEvent, StopToken};
use reqwest::Response;
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::time::timeout;

const MAXIMUM_ERROR_MESSAGE_CHARACTERS: usize = 4096;

pub(super) struct RoundResult {
    pub(super) tool_calls: Vec<NativeToolCall>,
    pub(super) content: String,
    pub(super) thinking: Option<String>,
    pub(super) finish_reason: FinishReason,
    pub(super) observed_input_tokens: u64,
}

pub(super) struct RoundStreamState {
    pub(super) content: String,
    pub(super) thinking: String,
    pub(super) tool_calls: BTreeMap<usize, NativeToolCall>,
    pub(super) is_complete: bool,
    pub(super) finish_reason: FinishReason,
    pub(super) observed_input_tokens: u64,
}

impl Default for RoundStreamState {
    fn default() -> Self {
        Self {
            content: String::new(),
            thinking: String::new(),
            tool_calls: BTreeMap::new(),
            is_complete: false,
            finish_reason: FinishReason::Stop,
            observed_input_tokens: 0,
        }
    }
}

pub(super) async fn stream_round(
    response: Response,
    host: &Arc<dyn ProviderRunHost>,
    stop: &StopToken,
    idle_timeout_milliseconds: Option<u64>,
    run_progress: &mut RunProgress,
) -> Result<RoundResult, LlmError> {
    let mut response_stream = response.bytes_stream();
    let mut pending_frame_bytes = Vec::new();
    let mut round_stream_state = RoundStreamState::default();

    'stream: loop {
        let next_chunk = tokio::select! {
            () = stop.cancelled() => return Err(LlmError::Cancelled),
            result = async {
                if let Some(timeout_milliseconds) = idle_timeout_milliseconds {
                    timeout(Duration::from_millis(timeout_milliseconds), response_stream.next())
                        .await
                        .map_err(|_| LlmError::Timeout("Ollama stream idle timeout".into()))
                } else {
                    Ok(response_stream.next().await)
                }
            } => result,
        }?;

        let Some(chunk) = next_chunk else {
            break;
        };

        pending_frame_bytes.extend_from_slice(&chunk.map_err(network_error)?);

        while let Some(newline_position) =
            pending_frame_bytes.iter().position(|byte| *byte == b'\n')
        {
            let frame_bytes = pending_frame_bytes
                .drain(..=newline_position)
                .collect::<Vec<_>>();
            let frame_bytes = &frame_bytes[..frame_bytes.len() - 1];

            if frame_bytes.iter().all(u8::is_ascii_whitespace) {
                continue;
            }

            process_frame_bytes(frame_bytes, host, run_progress, &mut round_stream_state).await?;

            if round_stream_state.is_complete {
                pending_frame_bytes.clear();
                break 'stream;
            }
        }
    }

    if !pending_frame_bytes.iter().all(u8::is_ascii_whitespace) {
        process_frame_bytes(
            &pending_frame_bytes,
            host,
            run_progress,
            &mut round_stream_state,
        )
        .await?;
    }

    if !round_stream_state.is_complete {
        return Err(LlmError::ProviderProtocol(
            "Ollama stream ended before done=true".into(),
        ));
    }

    host.emit(RunEvent::Usage(run_progress.usage.clone()))
        .await?;

    if !round_stream_state.tool_calls.is_empty()
        && round_stream_state.finish_reason == FinishReason::Stop
    {
        round_stream_state.finish_reason = FinishReason::ToolCalls;
    }

    Ok(RoundResult {
        tool_calls: round_stream_state.tool_calls.into_values().collect(),
        content: round_stream_state.content,
        thinking: (!round_stream_state.thinking.is_empty()).then_some(round_stream_state.thinking),
        finish_reason: round_stream_state.finish_reason,
        observed_input_tokens: round_stream_state.observed_input_tokens,
    })
}

async fn process_frame_bytes(
    frame_bytes: &[u8],
    host: &Arc<dyn ProviderRunHost>,
    run_progress: &mut RunProgress,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    let frame = serde_json::from_slice(frame_bytes)
        .map_err(|_| LlmError::ProviderProtocol("invalid Ollama NDJSON frame".into()))?;

    process_frame(&frame, host, run_progress, round_stream_state).await
}

async fn process_frame(
    frame: &Value,
    host: &Arc<dyn ProviderRunHost>,
    run_progress: &mut RunProgress,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    if let Some(error) = frame.get("error") {
        return Err(native_error_frame(error));
    }

    if let Some(message) = frame.get("message") {
        if let Some(content) = message.get("content").and_then(Value::as_str) {
            append_response_delta(content, host, run_progress, round_stream_state).await?;
        }

        if let Some(thinking) = message.get("thinking").and_then(Value::as_str) {
            append_reasoning_delta(thinking, host, run_progress, round_stream_state).await?;
        }

        if let Some(native_calls) = message.get("tool_calls").and_then(Value::as_array) {
            merge_native_tool_calls(native_calls, &mut round_stream_state.tool_calls)?;
        }
    }

    if frame.get("done").and_then(Value::as_bool) == Some(true) {
        round_stream_state.is_complete = true;
        round_stream_state.finish_reason = finish_reason_from_native(frame.get("done_reason"));
        record_completed_frame_usage(frame, run_progress, round_stream_state);
    }

    Ok(())
}

fn native_error_frame(error: &Value) -> LlmError {
    let error_message = error
        .as_str()
        .map(ToString::to_string)
        .or_else(|| {
            error
                .get("message")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
        .or_else(|| serde_json::to_string(error).ok())
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
    run_progress: &mut RunProgress,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    if response_delta.is_empty() {
        return Ok(());
    }

    run_progress.response_text.push_str(response_delta);
    round_stream_state.content.push_str(response_delta);
    host.emit(RunEvent::ResponseDelta(response_delta.into()))
        .await
}

async fn append_reasoning_delta(
    reasoning_delta: &str,
    host: &Arc<dyn ProviderRunHost>,
    run_progress: &mut RunProgress,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    if reasoning_delta.is_empty() {
        return Ok(());
    }

    run_progress.visible_reasoning.push_str(reasoning_delta);
    round_stream_state.thinking.push_str(reasoning_delta);
    host.emit(RunEvent::ReasoningSummaryDelta(reasoning_delta.into()))
        .await
}
