use super::*;

pub(crate) async fn stream_round(
    response: Response,
    host: &Arc<dyn ProviderRunHost>,
    stop: &StopToken,
    idle_timeout_milliseconds: Option<u64>,
    assistant_round: &mut AssistantRound,
    aggregate_usage: &mut Usage,
) -> Result<RoundResult, LlmError> {
    let mut response_stream = response.bytes_stream();
    let mut pending_frame_bytes = Vec::new();
    let mut round_stream_state = RoundStreamState::default();

    loop {
        let next_chunk_outcome = if let Some(timeout_milliseconds) = idle_timeout_milliseconds {
            stop.run_until_cancelled_or_timeout(Duration::from_millis(timeout_milliseconds), response_stream.next())
                .await
        } else {
            stop.run_until_cancelled(response_stream.next()).await
        };

        let next_chunk = match next_chunk_outcome {
            ControlledFutureOutcome::Cancelled => return Err(LlmError::Cancelled),
            ControlledFutureOutcome::TimedOut => {
                return Err(LlmError::Timeout("Ollama stream idle timeout".into()));
            }
            ControlledFutureOutcome::Completed(next_chunk) => next_chunk,
        };

        let Some(chunk) = next_chunk else {
            break;
        };

        pending_frame_bytes.extend_from_slice(&chunk.map_err(network_error)?);

        let mut consumed_byte_count = 0_usize;

        while let Some(relative_newline_position) = pending_frame_bytes[consumed_byte_count..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            if stop.is_stopped() {
                return Err(LlmError::Cancelled);
            }

            let newline_position = consumed_byte_count
                .checked_add(relative_newline_position)
                .ok_or_else(|| LlmError::ProviderProtocol("Ollama frame offset overflow".into()))?;
            let frame_bytes = &pending_frame_bytes[consumed_byte_count..newline_position];

            if frame_bytes.len() > MAXIMUM_FRAME_BYTE_COUNT {
                return Err(LlmError::ProviderProtocol(
                    "Ollama NDJSON frame exceeded the size limit".into(),
                ));
            }

            if !frame_bytes.iter().all(u8::is_ascii_whitespace) {
                if round_stream_state.is_complete {
                    return Err(LlmError::ProviderProtocol(
                        "Ollama returned data after done=true".into(),
                    ));
                }

                process_frame_bytes(
                    frame_bytes,
                    host,
                    assistant_round,
                    aggregate_usage,
                    &mut round_stream_state,
                )
                .await?;
            }

            consumed_byte_count = newline_position
                .checked_add(1)
                .ok_or_else(|| LlmError::ProviderProtocol("Ollama frame offset overflow".into()))?;
        }

        if consumed_byte_count > 0 {
            pending_frame_bytes.drain(..consumed_byte_count);
        }

        if pending_frame_bytes.len() > MAXIMUM_FRAME_BYTE_COUNT {
            return Err(LlmError::ProviderProtocol(
                "Ollama NDJSON frame exceeded the size limit".into(),
            ));
        }
    }

    if !pending_frame_bytes.iter().all(u8::is_ascii_whitespace) && round_stream_state.is_complete {
        return Err(LlmError::ProviderProtocol(
            "Ollama returned data after done=true".into(),
        ));
    }

    if !pending_frame_bytes.iter().all(u8::is_ascii_whitespace) {
        process_frame_bytes(
            &pending_frame_bytes,
            host,
            assistant_round,
            aggregate_usage,
            &mut round_stream_state,
        )
        .await?;
    }

    if !round_stream_state.is_complete {
        return Err(LlmError::ProviderProtocol(
            "Ollama stream ended before done=true".into(),
        ));
    }

    if !round_stream_state.tool_calls.is_empty() {
        if round_stream_state.finish_reason == FinishReason::Stop {
            round_stream_state.finish_reason = FinishReason::ToolCalls;
        } else if round_stream_state.finish_reason != FinishReason::ToolCalls {
            return Err(LlmError::ProviderProtocol(
                "Ollama returned incomplete tool calls".into(),
            ));
        }
    }

    assistant_round.finish_reason = round_stream_state.finish_reason;
    host.emit(RunEvent::Usage(aggregate_usage.clone())).await?;

    Ok(RoundResult {
        tool_calls: round_stream_state.tool_calls.into_values().collect(),
        finish_reason: round_stream_state.finish_reason,
        observed_input_tokens: round_stream_state.observed_input_tokens,
    })
}

async fn process_frame_bytes(
    frame_bytes: &[u8],
    host: &Arc<dyn ProviderRunHost>,
    assistant_round: &mut AssistantRound,
    aggregate_usage: &mut Usage,
    round_stream_state: &mut RoundStreamState,
) -> Result<(), LlmError> {
    let frame = serde_json::from_slice::<NativeChatFrame>(frame_bytes)
        .map_err(|_| LlmError::ProviderProtocol("invalid Ollama NDJSON frame".into()))?;

    process_frame(&frame, host, assistant_round, aggregate_usage, round_stream_state).await
}
