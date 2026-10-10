use super::*;

pub(super) async fn read_bounded_json_body(
    mut response: Response,
    stop: Option<&llm::StopToken>,
) -> Result<Vec<u8>, llm::LlmError> {
    let mut response_body = Vec::new();

    loop {
        let response_chunk = if let Some(stop_token) = stop {
            match stop_token.run_until_cancelled(response.chunk()).await {
                llm::ControlledFutureOutcome::Cancelled => {
                    return Err(llm::LlmError::Cancelled);
                }
                llm::ControlledFutureOutcome::TimedOut => unreachable!(),
                llm::ControlledFutureOutcome::Completed(result) => {
                    result.map_err(protocol_error)?
                }
            }
        } else {
            response.chunk().await.map_err(protocol_error)?
        };

        let Some(response_chunk) = response_chunk else {
            break;
        };

        let updated_byte_count = response_body
            .len()
            .checked_add(response_chunk.len())
            .ok_or_else(|| {
                llm::LlmError::ProviderProtocol("Ollama response size overflow".into())
            })?;

        if updated_byte_count > MAXIMUM_JSON_RESPONSE_BYTE_COUNT {
            return Err(llm::LlmError::ProviderProtocol(
                "Ollama response exceeded the size limit".into(),
            ));
        }

        response_body.extend_from_slice(&response_chunk);
    }

    Ok(response_body)
}
