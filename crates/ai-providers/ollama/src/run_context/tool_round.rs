use super::ModelRunState;
use crate::{
    content_history::native_messages_from_history, request::estimate_serialized_tokens,
    run_request::optimize_context_if_needed, streaming::RoundResult, streaming_tools::normalize_tool_call,
};
use llm::{AssistantRound, LlmError, ProviderRunHost, ProviderRunRequest, RunEvent};
use std::sync::Arc;

pub(crate) async fn continue_after_tool_round(
    request: &ProviderRunRequest,
    host: &Arc<dyn ProviderRunHost>,
    round_number: u16,
    round_result: &RoundResult,
    assistant_round: &mut AssistantRound,
    model_run_state: &mut ModelRunState,
) -> Result<(), LlmError> {
    let tool_calls_result = round_result
        .tool_calls
        .iter()
        .cloned()
        .enumerate()
        .map(|(call_index, native_tool_call)| {
            normalize_tool_call(
                native_tool_call,
                request.run_id,
                round_number,
                call_index,
                &mut model_run_state.observed_tool_call_identifiers,
            )
        })
        .collect::<Result<Vec<_>, _>>();

    let tool_calls = match tool_calls_result {
        Ok(tool_calls) => tool_calls,
        Err(error) => {
            emit_provider_round_finished(host, assistant_round).await?;
            return Err(error);
        }
    };

    assistant_round.tool_calls.clone_from(&tool_calls);
    emit_provider_round_finished(host, assistant_round).await?;

    let tool_outputs = host.execute_tool_round(round_number, tool_calls.clone()).await?;

    model_run_state
        .conversation_history
        .append_round(assistant_round, &tool_calls, &tool_outputs)?;

    if round_number < request.limits.provider_rounds {
        let observed_input_tokens = observed_or_estimated_input_tokens(
            round_result.observed_input_tokens,
            &model_run_state.conversation_history.model_visible,
        )?;

        optimize_context_if_needed(
            request,
            host,
            &mut model_run_state.conversation_history,
            model_run_state.input_token_limit,
            observed_input_tokens,
            estimate_serialized_tokens(&tool_outputs)?,
        )
        .await?;
    }

    model_run_state.native_messages = native_messages_from_history(
        request.system_prompt.as_deref(),
        &model_run_state.conversation_history.model_visible,
        model_run_state.model_is_gemma_four,
    )
    .await?;

    Ok(())
}

pub(crate) async fn emit_provider_round_finished(
    host: &Arc<dyn ProviderRunHost>,
    assistant_round: &AssistantRound,
) -> Result<(), LlmError> {
    host.emit(RunEvent::ProviderRoundFinished {
        round: assistant_round.round_number,
        finish_reason: assistant_round.finish_reason,
        tool_calls: assistant_round.tool_calls.clone(),
        usage: assistant_round.usage.clone(),
    })
    .await
}

fn observed_or_estimated_input_tokens(
    observed_input_tokens: u64,
    model_visible_history: &[llm::ConversationMessage],
) -> Result<u64, LlmError> {
    (observed_input_tokens > 0)
        .then_some(Ok(observed_input_tokens))
        .unwrap_or(estimate_serialized_tokens(model_visible_history))
}
